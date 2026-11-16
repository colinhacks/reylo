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
    /// holds this check of its `.sql` file against the text it was written from. A carriage return
    /// before a line feed is passed over, so a checkout that writes lines as CRLF, as git does on
    /// Windows, reads as the text the module was written from.
    pub const fn same_text(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        let (mut i, mut j) = (0, 0);
        loop {
            if i + 1 < a.len() && a[i] == b'\r' && a[i + 1] == b'\n' {
                i += 1;
            }
            if j + 1 < b.len() && b[j] == b'\r' && b[j + 1] == b'\n' {
                j += 1;
            }
            if i == a.len() || j == b.len() {
                return i == a.len() && j == b.len();
            }
            if a[i] != b[j] {
                return false;
            }
            i += 1;
            j += 1;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::same_text;

        #[test]
        fn a_text_is_the_same_whichever_way_its_lines_end() {
            assert!(same_text("select 1\nfrom t\n", "select 1\nfrom t\n"));
            assert!(same_text("select 1\r\nfrom t\r\n", "select 1\nfrom t\n"));
            assert!(same_text("select 1\nfrom t\n", "select 1\r\nfrom t\r\n"));
            assert!(same_text("", ""));
            assert!(!same_text("select 1\n", "select 2\n"));
            assert!(!same_text("select 1", "select 1\n"));
            assert!(!same_text("a\rb", "a\nb"));
        }
    }
}
