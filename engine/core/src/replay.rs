//! The replay run to its end, for a host that links the engine natively: call a round, answer
//! every need it names, call again, until the engine answers with the analysis. [`run`] does it
//! for an [`Answerer`] that answers as it is called, and [`run_async`] for one whose answers are
//! futures, as a database client's are. Each round runs on the engine's stack, as [`crate::api::call`]
//! runs one.
//!
//! A host keeps the answers it has across statements in an [`AnswerCache`]: every need is named by
//! what it asks (`describe:<sql>`, `query:<sql>|<params>`), so a later statement that asks the same
//! is answered without a round trip, for as long as the answer's [`Scope`] says it stays true.

use crate::api::{Analysis, Answer, Answers, DbError, Need, Response, Scope};
use crate::database::Database;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

/// How many rounds a statement may take before the replay is given up as unsettled. A statement
/// settles in a handful: each round collects every need that does not wait on another's answer.
pub const ROUNDS: usize = 64;

/// What answers the engine's needs as it is called: a database client that blocks.
pub trait Answerer<D> {
    /// Why the host could not answer at all, such as a lost connection; it ends the replay. A
    /// database's refusal of what it was asked is not one: that is the inner `Err`, an answer the
    /// engine reads (an unknown column in a describe becomes a diagnostic).
    type Error;

    /// The database's describe of `sql`: Postgres's Parse and Describe, MySQL's prepare; never
    /// asked of SQLite, which prepares inside the engine.
    fn describe(&mut self, sql: &str) -> Result<Result<D, DbError>, Self::Error>;

    /// A catalog query's rows, each an array of its values in column order.
    fn query(&mut self, sql: &str, params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, Self::Error>;
}

/// [`Answerer`] for a client whose answers are futures.
pub trait AsyncAnswerer<D> {
    /// As [`Answerer::Error`].
    type Error;

    /// As [`Answerer::describe`].
    fn describe(&mut self, sql: &str) -> impl Future<Output = Result<Result<D, DbError>, Self::Error>> + Send;

    /// As [`Answerer::query`].
    fn query(&mut self, sql: &str, params: &[Value]) -> impl Future<Output = Result<Result<Vec<Vec<Value>>, DbError>, Self::Error>> + Send;
}

/// Why a replay ended without an analysis.
#[derive(Debug)]
pub enum ReplayError<E> {
    /// The host could not answer.
    Host(E),
    /// The engine failed, with its message.
    Engine(String),
    /// The engine was still asking after [`ROUNDS`] rounds.
    Unsettled,
}

impl<E: std::fmt::Display> std::fmt::Display for ReplayError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Host(e) => write!(f, "{e}"),
            ReplayError::Engine(message) => write!(f, "{message}"),
            ReplayError::Unsettled => write!(f, "the engine was still asking after {ROUNDS} rounds"),
        }
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for ReplayError<E> {}

/// The answers a host keeps across statements. A session answer (`Scope::Session`: a server
/// setting, a type the database defines) stays for `session`, any other for `statement`, so a
/// migration shows up within seconds and a statement analyzed again a moment later costs no round
/// trip. A failure is never kept: the next statement asks again.
pub struct AnswerCache<D> {
    entries: HashMap<String, (Instant, Answer<D>)>,
    session: Duration,
    statement: Duration,
}

impl<D> Default for AnswerCache<D> {
    fn default() -> Self {
        AnswerCache::new(Duration::from_secs(60), Duration::from_secs(5))
    }
}

impl<D> AnswerCache<D> {
    pub fn new(session: Duration, statement: Duration) -> Self {
        AnswerCache { entries: HashMap::new(), session, statement }
    }

    /// Forgets every answer, as after a change to the schema.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn get(&self, need: &Need) -> Option<&Answer<D>> {
        let ttl = match need {
            Need::Query { scope: Scope::Session, .. } => self.session,
            _ => self.statement,
        };
        self.entries.get(id(need)).filter(|(at, _)| at.elapsed() < ttl).map(|(_, answer)| answer)
    }

    fn put(&mut self, need: &Need, answer: &Answer<D>)
    where
        D: Clone,
    {
        if !matches!(answer, Answer::Error { .. }) {
            self.entries.insert(id(need).to_string(), (Instant::now(), answer.clone()));
        }
    }
}

fn id(need: &Need) -> &str {
    match need {
        Need::Describe { id, .. } | Need::Query { id, .. } => id,
    }
}

/// Where a round left the replay.
enum Round {
    Done(Box<Analysis>),
    Needs(Vec<Need>),
}

/// One round over the answers so far, on the engine's stack.
fn round<Db: Database>(sql: &str, answers: &mut Answers<Db::Describe>) -> Result<Round, String> {
    Db::unbundle(answers);
    let answers: &Answers<Db::Describe> = answers;
    match crate::stack::run(move || Db::round(sql, answers)) {
        Response::Analysis { analysis } => Ok(Round::Done(Box::new(analysis))),
        Response::Need { needs } => Ok(Round::Needs(needs)),
        Response::Error { message } => Err(message),
        other => Err(format!("the engine answered a request for an analysis with {other:?}")),
    }
}

fn answer<D, E>(answered: Result<Result<D, DbError>, E>) -> Result<Answer<D>, ReplayError<E>> {
    Ok(match answered.map_err(ReplayError::Host)? {
        Ok(describe) => Answer::Describe { describe },
        Err(error) => Answer::Error { error },
    })
}

fn rows<D, E>(answered: Result<Result<Vec<Vec<Value>>, DbError>, E>) -> Result<Answer<D>, ReplayError<E>> {
    Ok(match answered.map_err(ReplayError::Host)? {
        Ok(rows) => Answer::Rows { rows },
        Err(error) => Answer::Error { error },
    })
}

/// The analysis of `sql`, each need answered from `cache` or by `answerer`.
pub fn run<Db: Database, A: Answerer<Db::Describe>>(sql: &str, answerer: &mut A, cache: &mut AnswerCache<Db::Describe>) -> Result<Analysis, ReplayError<A::Error>> {
    let mut answers: Answers<Db::Describe> = HashMap::new();
    for _ in 0..ROUNDS {
        let needs = match round::<Db>(sql, &mut answers).map_err(ReplayError::Engine)? {
            Round::Done(analysis) => return Ok(*analysis),
            Round::Needs(needs) => needs,
        };
        for need in needs {
            let known = cache.get(&need).cloned();
            let given = match known {
                Some(known) => known,
                None => {
                    let fresh = match &need {
                        Need::Describe { sql, .. } => answer(answerer.describe(sql))?,
                        Need::Query { sql, params, .. } => rows(answerer.query(sql, params))?,
                    };
                    cache.put(&need, &fresh);
                    fresh
                }
            };
            answers.insert(id(&need).to_string(), given);
        }
    }
    Err(ReplayError::Unsettled)
}

/// [`run`] for an [`AsyncAnswerer`].
pub async fn run_async<Db: Database, A: AsyncAnswerer<Db::Describe>>(sql: &str, answerer: &mut A, cache: &mut AnswerCache<Db::Describe>) -> Result<Analysis, ReplayError<A::Error>> {
    let mut answers: Answers<Db::Describe> = HashMap::new();
    for _ in 0..ROUNDS {
        let needs = match round::<Db>(sql, &mut answers).map_err(ReplayError::Engine)? {
            Round::Done(analysis) => return Ok(*analysis),
            Round::Needs(needs) => needs,
        };
        for need in needs {
            let known = cache.get(&need).cloned();
            let given = match known {
                Some(known) => known,
                None => {
                    let fresh = match &need {
                        Need::Describe { sql, .. } => answer(answerer.describe(sql).await)?,
                        Need::Query { sql, params, .. } => rows(answerer.query(sql, params).await)?,
                    };
                    cache.put(&need, &fresh);
                    fresh
                }
            };
            answers.insert(id(&need).to_string(), given);
        }
    }
    Err(ReplayError::Unsettled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Diagnostic;
    use crate::dialect::Dialect;
    use crate::host::{Host, Lazy};
    use serde_json::json;

    /// A database that asks, in one round, for the statement's describe and the server's version,
    /// and puts both answers in the analysis's one diagnostic.
    struct Toy;

    impl Database for Toy {
        const DIALECT: Dialect = Dialect::Postgres;
        const FINGERPRINT: &'static str = "toy";
        type Describe = String;

        fn analyze(sql: &str, host: &Host<'_, String>) -> Lazy<Analysis> {
            let described = host.describe(sql);
            let version = host.query("select version()", vec![], Scope::Session);
            let (described, version) = (described?, version?);
            let described = described.unwrap_or_else(|e| e.message);
            let version = version.map(|rows| rows[0][0].to_string()).unwrap_or_else(|e| e.message);
            Some(crate::analysis::bare_of(sql, Dialect::Postgres, vec![Diagnostic { message: format!("{described} | {version}"), start: 0, length: 0, code: 0 }]))
        }
    }

    #[derive(Default)]
    struct Counting {
        describes: usize,
        queries: usize,
        lost: bool,
    }

    impl Counting {
        fn describe_now(&mut self, sql: &str) -> Result<Result<String, DbError>, String> {
            self.describes += 1;
            if sql.contains("nope") {
                return Ok(Err(DbError { message: "relation \"nope\" does not exist".into(), code: Some("42P01".into()), position: None }));
            }
            Ok(Ok(format!("described {sql}")))
        }

        fn query_now(&mut self) -> Result<Result<Vec<Vec<Value>>, DbError>, String> {
            self.queries += 1;
            if self.lost {
                return Err("connection lost".into());
            }
            Ok(Ok(vec![vec![json!("18.0")]]))
        }
    }

    impl Answerer<String> for Counting {
        type Error = String;
        fn describe(&mut self, sql: &str) -> Result<Result<String, DbError>, String> {
            self.describe_now(sql)
        }
        fn query(&mut self, _sql: &str, _params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, String> {
            self.query_now()
        }
    }

    impl AsyncAnswerer<String> for Counting {
        type Error = String;
        fn describe(&mut self, sql: &str) -> impl Future<Output = Result<Result<String, DbError>, String>> + Send {
            let answered = self.describe_now(sql);
            async move { answered }
        }
        fn query(&mut self, _sql: &str, _params: &[Value]) -> impl Future<Output = Result<Result<Vec<Vec<Value>>, DbError>, String>> + Send {
            let answered = self.query_now();
            async move { answered }
        }
    }

    fn said(analysis: &Analysis) -> &str {
        &analysis.diagnostics[0].message
    }

    /// Polls a future whose every await is ready, as the answers above are.
    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("an answer was not ready"),
        }
    }

    #[test]
    fn answers_every_need_and_keeps_what_stays_true() {
        let mut host = Counting::default();
        let mut cache = AnswerCache::default();
        let first = run::<Toy, _>("select 1", &mut host, &mut cache).unwrap();
        assert_eq!(said(&first), "described select 1 | \"18.0\"");
        assert_eq!((host.describes, host.queries), (1, 1));
        // The same statement again: both answers from the cache.
        run::<Toy, _>("select 1", &mut host, &mut cache).unwrap();
        assert_eq!((host.describes, host.queries), (1, 1));
        // Another statement: its own describe, the session's version kept.
        let other = run::<Toy, _>("select 2", &mut host, &mut cache).unwrap();
        assert_eq!(said(&other), "described select 2 | \"18.0\"");
        assert_eq!((host.describes, host.queries), (2, 1));
        // A statement answer stays for `statement`, a session one for `session`.
        let mut brief = AnswerCache::new(Duration::from_secs(60), Duration::ZERO);
        run::<Toy, _>("select 3", &mut host, &mut brief).unwrap();
        run::<Toy, _>("select 3", &mut host, &mut brief).unwrap();
        assert_eq!((host.describes, host.queries), (4, 2));
    }

    #[test]
    fn a_refusal_is_an_answer_and_a_lost_host_ends_the_replay() {
        let mut host = Counting::default();
        let mut cache = AnswerCache::default();
        let refused = run::<Toy, _>("select * from nope", &mut host, &mut cache).unwrap();
        assert_eq!(said(&refused), "relation \"nope\" does not exist | \"18.0\"");
        // A refusal is not kept: the next statement that asks asks the database again.
        run::<Toy, _>("select * from nope", &mut host, &mut cache).unwrap();
        assert_eq!(host.describes, 2);
        let mut lost = Counting { lost: true, ..Counting::default() };
        match run::<Toy, _>("select 1", &mut lost, &mut AnswerCache::default()) {
            Err(ReplayError::Host(e)) => assert_eq!(e, "connection lost"),
            other => panic!("expected the host's error, got {:?}", other.map(|a| a.diagnostics)),
        }
    }

    #[test]
    fn the_async_replay_answers_as_the_blocking_one() {
        let mut host = Counting::default();
        let mut cache = AnswerCache::default();
        let analysis = ready(run_async::<Toy, _>("select 1", &mut host, &mut cache)).unwrap();
        assert_eq!(said(&analysis), "described select 1 | \"18.0\"");
        ready(run_async::<Toy, _>("select 1", &mut host, &mut cache)).unwrap();
        assert_eq!((host.describes, host.queries), (1, 1));
    }
}
