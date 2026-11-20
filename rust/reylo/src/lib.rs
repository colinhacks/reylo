//! Reylo for Rust: `#[reylo::query("…")] pub mod get_user {}` types a statement with
//! Reylo's engine at compile time and fills the module with `Params` and `Row`. The query runs on
//! sqlx, on the caller's own pool, connection or transaction:
//! `get_user::Params { id: 2 }.run(&pool)` returns what the statement's cardinality says, so a key
//! lookup is an `Option<get_user::Row>` and a `count(*)` one `Row`, with no fetch method chosen.
//!
//! Each database enum a query reads or binds is an enum in its module, which implements [`Enum`];
//! `#[reylo::reflect("…")]` makes one of the crate's own enums one too.

pub use reylo_macros::{query, reflect};

#[cfg(feature = "migrate")]
pub mod migrate;
/// The project's migrations, compiled in: `reylo::migrate!().run(&pool)` runs `reylo migrate deploy`
/// from the app, and `.status(&pool)` `reylo migrate status` ([`migrate::Migrator`]).
#[cfg(feature = "migrate")]
pub use reylo_macros::migrate;

mod enums;
mod exact;
pub use enums::{Covers, Enum, EnumId, HasLabel, HasLabels, Label, Labels, UnexpectedRow};
pub use exact::Exact;

#[cfg(all(feature = "sqlx_0_9", feature = "sqlx_0_8"))]
compile_error!("reylo's sqlx_0_9 and sqlx_0_8 features are both on: turn on the one of the sqlx the crate runs on, with `default-features = false` for sqlx_0_8");
#[cfg(not(any(feature = "sqlx_0_9", feature = "sqlx_0_8")))]
compile_error!("reylo needs the feature of the sqlx the crate runs on: sqlx_0_9 (a default feature) or sqlx_0_8");
#[cfg(all(feature = "sqlx_0_8", feature = "sqlite"))]
compile_error!("reylo's sqlite feature runs on sqlx 0.9 alone: sqlx 0.8's SQLite links libsqlite3-sys 0.30, and the macro's SQLite client a newer one, which Cargo cannot build beside it");

// What the expansion names, so that a crate needs no dependency of its own on them.
pub use serde;
pub use serde_json;
#[cfg(feature = "sqlx_0_9")]
pub use sqlx;
#[cfg(all(feature = "sqlx_0_8", not(feature = "sqlx_0_9")))]
pub use sqlx08 as sqlx;

#[doc(hidden)]
pub mod __private {
    use crate::sqlx;
    use futures_util::TryStreamExt;

    /// The buffer a database's `Encode` writes into: sqlx 0.8 names it with the lifetime of the
    /// query it is bound to, and sqlx 0.9 without.
    #[cfg(feature = "sqlx_0_9")]
    pub type ArgumentBuffer<'q, DB> = <DB as sqlx::Database>::ArgumentBuffer;
    #[cfg(all(feature = "sqlx_0_8", not(feature = "sqlx_0_9")))]
    pub type ArgumentBuffer<'q, DB> = <DB as sqlx::Database>::ArgumentBuffer<'q>;

    pub use futures_core::stream::BoxStream;

    /// Whether a value of the enum of `a_type` and `a_labels` casts into the enum of `b_type` and
    /// `b_labels`, in a constant: a Postgres enum type's into the same type with the same values in
    /// the same order, and a list's (no type) into any enum that has each of its values.
    pub const fn casts_into(a_type: &str, a_labels: &[&str], b_type: &str, b_labels: &[&str]) -> bool {
        if !a_type.is_empty() {
            return same_enum(a_type, a_labels, b_type, b_labels);
        }
        let mut i = 0;
        while i < a_labels.len() {
            let mut j = 0;
            while j < b_labels.len() && !same_str(a_labels[i], b_labels[j]) {
                j += 1;
            }
            if j == b_labels.len() {
                return false;
            }
            i += 1;
        }
        true
    }

    /// FNV-1a over a value's text and a zero byte: the value as a type, `reylo::Label`.
    pub const fn label_hash(label: &str) -> u64 {
        let bytes = label.as_bytes();
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut i = 0;
        while i <= bytes.len() {
            hash ^= if i < bytes.len() { bytes[i] as u64 } else { 0 };
            hash = hash.wrapping_mul(0x0100_0000_01b3);
            i += 1;
        }
        hash
    }

    /// Whether two enums are the same database enum, in a constant: the same type, and the same
    /// values in the same order.
    const fn same_enum(a_type: &str, a_labels: &[&str], b_type: &str, b_labels: &[&str]) -> bool {
        if !same_str(a_type, b_type) || a_labels.len() != b_labels.len() {
            return false;
        }
        let mut i = 0;
        while i < a_labels.len() {
            if !same_str(a_labels[i], b_labels[i]) {
                return false;
            }
            i += 1;
        }
        true
    }

    const fn same_str(a: &str, b: &str) -> bool {
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

    /// A column's value as one of an enum's variants, or the error that names the column and the
    /// value, which is no value of the enum.
    pub fn from_label<T: crate::Enum>(label: &str, column: &str) -> sqlx::Result<T> {
        T::from_label(label).ok_or_else(|| sqlx::Error::ColumnDecode {
            index: column.to_string(),
            source: format!("{label:?} is no value of the enum Reylo inferred for the column ({})", T::LABELS.join(", ")).into(),
        })
    }

    /// The statement's one row: none is sqlx's `RowNotFound`, and a second an [`crate::UnexpectedRow`].
    pub async fn exactly_one<R>(rows: BoxStream<'_, sqlx::Result<R>>, sql: &'static str) -> sqlx::Result<R> {
        at_most_one(rows, sql).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// The statement's row if it returned one; a second is an [`crate::UnexpectedRow`].
    pub async fn at_most_one<R>(mut rows: BoxStream<'_, sqlx::Result<R>>, sql: &'static str) -> sqlx::Result<Option<R>> {
        let Some(first) = rows.try_next().await? else { return Ok(None) };
        if rows.try_next().await?.is_some() {
            return Err(sqlx::Error::Decode(Box::new(crate::UnexpectedRow { sql })));
        }
        Ok(Some(first))
    }

    /// The statement's rows, each decoded as it arrives.
    pub fn rows<'e, R: Send + 'e, T: Send + 'e>(rows: BoxStream<'e, sqlx::Result<R>>, decode: fn(&R) -> sqlx::Result<T>) -> BoxStream<'e, sqlx::Result<T>> {
        Box::pin(rows.and_then(move |row| std::future::ready(decode(&row))))
    }

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
        use super::{casts_into, label_hash, same_text};

        #[test]
        fn a_list_casts_into_any_enum_with_each_of_its_values_and_a_type_only_into_itself() {
            assert!(casts_into("", &["done", "todo"], "", &["todo", "doing", "done"]));
            assert!(casts_into("", &["sad", "ok"], "public.mood", &["ok", "sad"]));
            assert!(!casts_into("", &["todo", "doing", "done"], "", &["done", "todo"]));
            assert!(casts_into("public.mood", &["ok", "sad"], "public.mood", &["ok", "sad"]));
            assert!(!casts_into("public.mood", &["ok", "sad"], "", &["ok", "sad"]));
            assert!(!casts_into("public.mood", &["ok", "sad"], "public.feeling", &["ok", "sad"]));
        }

        #[test]
        fn a_label_hash_is_fnv_1a_of_the_text_and_a_zero_byte() {
            assert_eq!(label_hash(""), 0xaf63_bd4c_8601_b7df);
            assert_ne!(label_hash("ab"), label_hash("a"));
            assert_ne!(label_hash("draft"), label_hash("Draft"));
        }

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
