//! [`Enum`]: a Rust enum whose variants are the values of one database enum, which every enum a query
//! module or `#[reylo::enum_type]` writes implements, and [`UnexpectedRow`], the error `.run()`
//! returns when a statement returns more rows than its cardinality allows.

/// A Rust enum whose variants are the values of one database enum: a Postgres enum type, or the list
/// of values a `CHECK` constraint or a MySQL `ENUM` allows a column, or the statement itself names.
///
/// Each query module writes its own enum for each one it reads or binds, and `#[reylo::enum_type]`
/// makes a crate's own enum one. [`Enum::cast`] turns a value of one into the same value of another,
/// and fails to compile where the two are not the same database enum.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not one of Reylo's enums",
    label = "a database enum's value goes here",
    note = "write `#[reylo::enum_type(\"<schema>.<type>\")]` on `{Self}`, or name the column in a partial `pub struct Row {{ … }}` in the query's module, which matches the enum's variants by name"
)]
pub trait Enum: Copy + 'static {
    /// The Postgres enum type the values are of, as `public.job_status`; empty for a list of values
    /// a `CHECK` constraint, a MySQL `ENUM` or the statement names.
    const TYPE: &'static str;
    /// The values, in the database's order, as it stores them.
    const LABELS: &'static [&'static str];

    /// The value as the database stores it.
    fn label(self) -> &'static str;

    /// The variant of a value, or none for a value that is not one of [`Enum::LABELS`].
    fn from_label(label: &str) -> Option<Self>;

    /// This value as another Reylo enum of the same database enum: the same Postgres type, or for a
    /// list of values, the same values in the same order. Any other pair fails to compile.
    ///
    /// ```ignore
    /// let next = next_job::Query.run(&pool).await?.unwrap();
    /// set_status::Query { status: next.status.cast(), id: next.id }.run(&pool).await?;
    /// ```
    fn cast<T: Enum>(self) -> T {
        const {
            assert!(
                crate::__private::same_enum(Self::TYPE, Self::LABELS, T::TYPE, T::LABELS),
                "cast between two enums of different database enums: their type or their values differ"
            )
        };
        match T::from_label(self.label()) {
            Some(value) => value,
            None => unreachable!("the two enums' labels were compared when the crate compiled"),
        }
    }
}

/// A statement returned a second row where Reylo inferred it returns one row, or at most one:
/// `.run()` returns it as `sqlx::Error::Decode`. It means a statement's cardinality was inferred
/// wrong, so the rows are not dropped quietly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnexpectedRow {
    /// The statement, as it was sent.
    pub sql: &'static str,
}

impl std::fmt::Display for UnexpectedRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the statement returned a second row, where Reylo inferred at most one: {}", self.sql)
    }
}

impl std::error::Error for UnexpectedRow {}
