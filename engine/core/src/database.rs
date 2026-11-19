//! The boundary between core and a database: what a database's crate implements so that
//! [`crate::api::call`] can serve its statements. The trait is used through static dispatch only:
//! each artifact instantiates `call::<D>` for exactly one `D`, and a native host that links several
//! databases picks one per request ([`crate::api::dialect_of`]).

use crate::api::{Analysis, Answers, Describe, Response};
use crate::dialect::Dialect;
use crate::errors::Errors;
use crate::host::{Host, Lazy};

pub trait Database {
    /// The name a request carries, and the placeholder rules the lexer applies.
    const DIALECT: Dialect;
    /// The sources this database's answers come from: core's and its own ([`crate::fingerprint`]).
    const FINGERPRINT: &'static str;
    /// What the host answers a `describe` need with.
    type Describe: Describe;

    /// The analysis of the statement as its author wrote it, from what the host has answered so
    /// far, or `None` with the missing needs recorded on `host`.
    fn analyze(sql: &str, host: &Host<'_, Self::Describe>) -> Lazy<Analysis>;

    /// The names each error the statement may raise can carry ([`crate::errors`]): `Some(None)`
    /// where the statement may run code the database does not define, or the database narrows none.
    fn errors(_analysis: &Analysis, _host: &Host<'_, Self::Describe>) -> Lazy<Option<Errors>> {
        Some(None)
    }

    /// Whether the engine finds nothing in the statement that could write, read once the rest of the analysis is
    /// settled: by default it is not.
    fn read_only(_analysis: &Analysis, _host: &Host<'_, Self::Describe>) -> Lazy<bool> {
        Some(false)
    }

    /// What running the statement may do, an upper bound ([`crate::types::Capabilities`]), read
    /// once the rest of the analysis is settled: by default nothing is said.
    fn capabilities(_analysis: &Analysis, _host: &Host<'_, Self::Describe>) -> Lazy<Option<crate::types::Capabilities>> {
        Some(None)
    }

    /// One request's round: the analysis, and then what it reads once the rest is settled. Postgres
    /// overrides it to settle what a server-side extension offers first.
    fn round(sql: &str, answers: &Answers<Self::Describe>) -> Response {
        let host = Host::new(answers);
        match Self::analyze(sql, &host) {
            Some(analysis) => with_facts::<Self>(analysis, &host),
            None => Response::Need { needs: host.needs() },
        }
    }

    /// Answers that came back in a form of the database's own, put where the analysis looks for
    /// them. By default nothing; Postgres unpacks a `reylo.answer` bundle.
    fn unbundle(_answers: &mut Answers<Self::Describe>) {}
}

/// The analysis with the errors it may raise, whether it writes nothing and what it may do, read in
/// one more round ([`crate::errors`]).
pub fn with_facts<D: Database + ?Sized>(mut analysis: Analysis, host: &Host<'_, D::Describe>) -> Response {
    let errors = if analysis.diagnostics.is_empty() { D::errors(&analysis, host) } else { Some(None) };
    match errors {
        Some(errors) => {
            analysis.errors = errors;
            match (D::read_only(&analysis, host), D::capabilities(&analysis, host)) {
                (Some(read_only), Some(capabilities)) => {
                    analysis.read_only = read_only;
                    analysis.capabilities = capabilities;
                    Response::Analysis { analysis }
                }
                _ => Response::Need { needs: host.needs() },
            }
        }
        None => Response::Need { needs: host.needs() },
    }
}
