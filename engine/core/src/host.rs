//! The engine's view of the database: answers the host already gave, and the needs it records
//! for the ones it has not. A lookup that misses records the need and answers `None`, and the
//! caller goes on with whatever else it can do this round, so one round collects every need
//! that does not depend on another's answer.
//!
//! The host is generic over what a `describe` need is answered with, which is the database's own:
//! Postgres's Parse/Describe, MySQL's prepare metadata, nothing for SQLite, which prepares inside
//! the engine. Each database crate names its host once (`pub type Host<'a> = …`).

use crate::api::{Answer, Answers, DbError, Describe, Need, Scope};
use serde_json::Value;
use std::cell::RefCell;

pub struct Host<'a, D> {
    answers: &'a Answers<D>,
    needs: RefCell<Vec<Need>>,
}

/// `None` is "not answered yet"; the need is recorded.
pub type Lazy<T> = Option<T>;

impl<'a, D: Describe> Host<'a, D> {
    pub fn new(answers: &'a Answers<D>) -> Host<'a, D> {
        Host { answers, needs: RefCell::new(Vec::new()) }
    }

    /// Run `f` over these answers on the engine's stack ([`crate::stack`]), with the needs it
    /// records recorded here.
    pub fn on_engine_stack<T: Send>(&self, f: impl FnOnce(&Host<'_, D>) -> T + Send) -> T {
        let answers = self.answers;
        let (value, needs) = crate::stack::run(move || {
            let host = Host::new(answers);
            let value = f(&host);
            (value, host.needs.into_inner())
        });
        for need in needs {
            self.record(need);
        }
        value
    }

    pub fn needs(&self) -> Vec<Need> {
        self.needs.borrow().clone()
    }

    pub fn pending(&self) -> bool {
        !self.needs.borrow().is_empty()
    }

    fn record(&self, need: Need) {
        let mut needs = self.needs.borrow_mut();
        if !needs.contains(&need) {
            needs.push(need);
        }
    }

    /// The database's describe of `sql`: its own payload, or the error the database answered with.
    pub fn describe(&self, sql: &str) -> Lazy<Result<D, DbError>> {
        let id = format!("describe:{sql}");
        match self.answers.get(&id) {
            Some(Answer::Describe { describe }) => Some(Ok(describe.clone())),
            Some(Answer::Error { error }) => Some(Err(error.clone())),
            Some(Answer::Rows { .. }) => Some(Err(DbError { message: "the host answered a describe with something else".into(), code: None, position: None })),
            None => {
                self.record(Need::Describe { id, sql: sql.to_string() });
                None
            }
        }
    }

    pub fn query(&self, sql: &str, params: Vec<Value>, scope: Scope) -> Lazy<Result<Vec<Vec<Value>>, DbError>> {
        let id = format!("query:{sql}|{}", serde_json::to_string(&params).unwrap_or_default());
        match self.answers.get(&id) {
            Some(Answer::Rows { rows }) => Some(Ok(rows.clone())),
            Some(Answer::Error { error }) => Some(Err(error.clone())),
            Some(Answer::Describe { .. }) => Some(Err(DbError { message: "the host answered a query with a describe".into(), code: None, position: None })),
            None => {
                self.record(Need::Query { id, sql: sql.to_string(), params, scope });
                None
            }
        }
    }

    /// A catalog query asked once per set of parameters, its rows together: the host keeps the
    /// answers by what each asks, so what a later statement asks again of the same relation costs
    /// no round trip, where a list of a statement's own relations would be a new question each time.
    /// `None` until each is answered; the first error otherwise.
    pub fn query_each(&self, sql: &str, each: impl IntoIterator<Item = Vec<Value>>, scope: Scope) -> Lazy<Result<Vec<Vec<Value>>, DbError>> {
        let mut asked = Vec::new();
        let (mut rows, mut error, mut pending) = (Vec::new(), None, false);
        for params in each {
            if asked.contains(&params) {
                continue;
            }
            asked.push(params.clone());
            match self.query(sql, params, scope) {
                Some(Ok(found)) => rows.extend(found),
                Some(Err(e)) => error = error.or(Some(e)),
                None => pending = true,
            }
        }
        if pending {
            return None;
        }
        Some(error.map_or(Ok(rows), Err))
    }
}

/// A number the driver may have decoded as a JSON number or, for `int8`, as a string.
pub fn as_u32(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().map(|n| n as u32),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

pub fn as_str(v: &Value) -> Option<&str> {
    v.as_str()
}

pub fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.as_str() {
            "t" | "true" => Some(true),
            "f" | "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub fn as_strings(v: &Value) -> Option<Vec<String>> {
    match v {
        Value::Array(items) => items.iter().map(|i| i.as_str().map(String::from)).collect(),
        Value::Null => None,
        _ => None,
    }
}
