//! [`Enum`]: a Rust enum whose variants are the values of one database enum, which every enum a query
//! module or `#[reylo::reflect]` writes implements, and [`UnexpectedRow`], the error `.run()`
//! returns when a statement returns more rows than its cardinality allows.

/// A Rust enum whose variants are the values of one database enum: a Postgres enum type, or the list
/// of values a `CHECK` constraint or a MySQL `ENUM` allows a column, or the statement itself names.
///
/// Each query module writes its own enum for each one it reads or binds, and `#[reylo::reflect]`
/// makes a crate's own enum one. [`Enum::cast`] turns a value of one into the same value of another,
/// and fails to type-check where the other cannot hold it: [`Enum::Id`] says as a type which enums
/// can, so `cargo check` and the editor see the mismatch too.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not one of Reylo's enums",
    label = "a database enum's value goes here",
    note = "write `#[reylo::reflect(\"<schema>.<type>\")]` on `{Self}`, or name the column in a partial `pub struct Row {{ … }}` in the query's module, which matches the enum's variants by name"
)]
pub trait Enum: Copy + 'static {
    /// Which enums a value casts into, as a type. For a Postgres enum type, [`EnumId`] of a hash of
    /// [`Enum::TYPE`] and [`Enum::LABELS`], the same for every Rust enum of the type. For a list of
    /// values, [`Labels`] of each value's [`Label`], so a value casts into any enum with a variant
    /// for each of them ([`HasLabel`]).
    type Id: 'static;
    /// The Postgres enum type the values are of, as `public.job_status`; empty for a list of values
    /// a `CHECK` constraint, a MySQL `ENUM` or the statement names.
    const TYPE: &'static str;
    /// The values, in the database's order, as it stores them.
    const LABELS: &'static [&'static str];

    /// The value as the database stores it.
    fn label(self) -> &'static str;

    /// The variant of a value, or none for a value that is not one of [`Enum::LABELS`].
    fn from_label(label: &str) -> Option<Self>;

    /// This value as another Reylo enum that holds it. A Postgres enum type's value casts into an
    /// enum of the same type, and a list's into any enum with a variant for each of its values: a
    /// list with more values, or a Postgres enum type that has them all. Any other pair fails to
    /// compile.
    ///
    /// ```ignore
    /// let next = next_job::Query.run(&pool).await?.unwrap();
    /// set_status::Query { status: next.status.cast(), id: next.id }.run(&pool).await?;
    /// ```
    fn cast<T: Enum + Covers<Self::Id>>(self) -> T {
        const {
            assert!(
                crate::__private::casts_into(Self::TYPE, Self::LABELS, T::TYPE, T::LABELS),
                "cast into an enum that cannot hold the value: another Postgres enum type, or one lacking a value of the list"
            )
        };
        match T::from_label(self.label()) {
            Some(value) => value,
            None => unreachable!("the two enums' labels were compared when the crate compiled"),
        }
    }
}

/// A Postgres enum type, as a type: the hash of its name and its values, which [`Enum::Id`] names.
#[doc(hidden)]
pub struct EnumId<const HASH: u64>;

/// A list of values, as a type, which [`Enum::Id`] names: each value's [`Label`], first to last, in
/// a balanced tree of pairs (one value is its `Label` alone, and more are a pair of the first half's
/// tree and the rest's). Proving a cast walks the tree, so its depth, about log2 of the count, is
/// what counts against the compiler's recursion limit, not the count.
#[doc(hidden)]
pub struct Labels<L>(core::marker::PhantomData<L>);

/// A value of a list, as a type: the hash of its text (`reylo::__private::label_hash`).
#[doc(hidden)]
pub struct Label<const HASH: u64>;

/// An enum with a variant for the value whose hash is `HASH`, which every Reylo enum implements for
/// each of its values, so that a list's value casts into it.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no variant for one of the values of the enum cast into it",
    label = "`{Self}` lacks one of this list's values",
    note = "a list of values casts into an enum with a variant for each of its values; compare the two enums' `LABELS`"
)]
pub trait HasLabel<const HASH: u64>: Enum {
    /// The variant.
    const VARIANT: Self;
}

/// What [`Enum::cast`] asks of the enum it casts into, given the `Id` of the one it casts from:
/// the same [`EnumId`], or for a list's [`Labels`], a variant for each value.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no variant for one of the values of the enum cast into it",
    label = "`{Self}` lacks one of this list's values",
    note = "a list of values casts into an enum with a variant for each of its values; compare the two enums' `LABELS`"
)]
pub trait Covers<Id> {}

impl<T: Enum<Id = EnumId<HASH>>, const HASH: u64> Covers<EnumId<HASH>> for T {}
impl<T: Enum + HasLabels<L>, L> Covers<Labels<L>> for T {}

/// An enum with a variant for each [`Label`] of `L`, a tree as [`Labels`] holds it.
#[doc(hidden)]
pub trait HasLabels<L> {}

impl<T> HasLabels<()> for T {}
impl<T: HasLabel<HASH>, const HASH: u64> HasLabels<Label<HASH>> for T {}
impl<T: HasLabels<A> + HasLabels<B>, A, B> HasLabels<(A, B)> for T {}

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
