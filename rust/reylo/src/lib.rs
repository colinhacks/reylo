//! Reylo for Rust: `#[reylo::query("…")] pub mod get_user {}` types a statement with
//! Reylo's engine at compile time and fills the module with `Params` and `Row`. The query runs on
//! sqlx, on the caller's own pool, connection or transaction:
//! `get_user::Params { id: 2 }.run(&pool)` returns what the statement's cardinality says, so a key
//! lookup is an `Option<get_user::Row>` and a `count(*)` one `Row`, with no fetch method chosen.

pub use reylo_macros::query;

mod exact;
pub use exact::Exact;

// What the expansion names, so that a crate needs no dependency of its own on them.
pub use serde;
pub use serde_json;
pub use sqlx;

#[doc(hidden)]
pub mod __private {
    /// Whether two texts are the same, in a constant: a module `reylo generate rust-sqlx` wrote
    /// holds this check of its `.sql` file against the text it was written from.
    pub const fn same_text(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}
