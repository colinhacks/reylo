//! The tables a statement reads and writes, as its database's own parse names them, which an
//! analysis records ([`crate::api::Analysis::tables`]) for the passes that read them after it, as
//! SQLite's errors pass does.

use serde::{Deserialize, Serialize};

/// The tables a statement reads and writes, as its database's own parse names them: each as the
/// statement writes it (a schema-qualified name with its schema), a WITH query's name never.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tables {
    pub read: Vec<String>,
    /// What an INSERT, an UPDATE, a DELETE or a MERGE writes.
    pub write: Vec<String>,
    /// The functions whose bodies an analysis read, as Postgres's `to_regprocedure` reads each
    /// back.
    pub functions: Vec<String>,
    /// The statement with those bodies in place.
    pub expanded: Option<String>,
}
