//! Reylo's Rust modules. A build script writes them into Cargo's `OUT_DIR` with `rust_sqlx()` (the
//! `generate` feature, a default one), so that no generated file is committed, and the crate
//! includes the file:
//!
//! ```ignore
//! // src/main.rs
//! mod queries {
//!     include!(concat!(env!("OUT_DIR"), "/queries.rs"));
//! }
//! ```
//!
//! The rest of the crate is what `#[reylo::query]` and `reylo generate rust-sqlx` run: a statement's
//! analysis as the Rust items of its module:
//!
//! - `Params`, a struct of the statement's parameters, one field per `:name` (a text parameter a
//!   `&str`), whose `run(executor)` runs the statement on sqlx and returns what the cardinality
//!   says: `Row` for one row, `Option<Row>` for at most one, `Vec<Row>` for many, and the affected
//!   row count for a write that returns none; `stream(executor)` for many rows, one at a time; and
//!   `Query`, the same struct by another name, so a call reads `get_user::Query { id: 2 }.run(&pool)`;
//! - `Row`, one row, each field typed from the column's type descriptor (`spec/`): a nullable
//!   column `Option<T>`, an array whose elements may be NULL `Vec<Option<T>>`, a
//!   `json_build_object` a struct of its fields; none for a write that returns no rows;
//! - `SQL`, the statement as it is sent;
//! - an enum per database enum the statement reads or binds, and a struct per JSON object it builds.
//!
//! The module's own items can stand in for some of these ([`Overrides`]): the crate's struct as the
//! whole row, or the crate's types for some of the row's or the parameters' fields.
//!
//! Every path the items name is written in full (`::core::option::Option`, `::reylo::sqlx`), since
//! the macro puts them beside the crate's own items, which may shadow a prelude name.

mod names;
mod types;

/// A statement's analysis, from the project's database or `reylo/.cache/`.
#[cfg(feature = "analyze")]
pub mod analysis;
/// The `.sql` files a path, a directory or a glob names, for `reylo generate rust-sqlx`.
#[cfg(feature = "generate")]
#[doc(hidden)]
pub mod files;
/// The modules written to files, as `reylo generate rust-sqlx` writes them; a build script's are
/// `rust_sqlx()`'s.
#[cfg(feature = "generate")]
#[doc(hidden)]
pub mod generate;
/// The modules a build script writes.
#[cfg(feature = "generate")]
mod builder;
#[cfg(feature = "generate")]
pub use builder::{rust_sqlx, RustSqlx};

/// Where a statement's byte stands in the string literal that holds it.
pub mod place;
/// The items as a file shows them, the prelude's names written short.
pub mod readable;

pub use names::variants;
pub use types::{enum_impl, enum_tokens, postgres_enum_tokens, postgres_impls, text_impls, SharedEnum, Types};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use reylo_engine_core::api::{Column, Param};
use serde_json::Value;
use std::collections::BTreeSet;
use syn::Ident;

/// The database a statement is written for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    Postgres,
    Sqlite,
    Mysql,
}

/// Which crate's types the items use for dates and times.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dates {
    Chrono,
    Time,
}

/// Which crate's type the items use for a decimal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decimals {
    RustDecimal,
    BigDecimal,
}

/// The crates the generated items may name beside sqlx, each enabled in `reylo` by the feature of
/// the same name. A statement whose types need one that is off fails, saying which to enable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Options {
    pub dates: Option<Dates>,
    pub uuid: bool,
    pub decimals: Option<Decimals>,
}

/// What the query's module, as the crate wrote it, holds in place of the generated items.
#[derive(Default)]
pub struct Overrides {
    /// The row: the crate's own struct (`pub use crate::jobs::QueryJob as Row;`), or some of its
    /// fields' types (`pub struct Row { pub status: crate::jobs::JobStatus }`).
    pub row: Option<RowOverride>,
    /// Some of the parameters' types (`pub struct Params { pub status: crate::jobs::JobStatus }`).
    pub params: Option<Partial>,
    /// The names of the crate's own items in the module, which the generated ones leave to them.
    pub taken: BTreeSet<String>,
}

pub enum RowOverride {
    /// The module names the crate's own struct `Row`, filled field by field: a column of an enum
    /// through `reylo::Enum::cast`, any other through `Into`.
    Whole,
    Partial(Partial),
}

/// A struct the crate wrote with some of the fields: its attributes, which stand in for the
/// generated ones, and those fields, whose types stand in for the generated ones.
pub struct Partial {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub fields: Vec<PartialField>,
    pub span: Span,
}

pub struct PartialField {
    pub name: String,
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub ty: syn::Type,
}

/// A statement's module: its items, and in a generated file, the Postgres enum types it reads from
/// the file's schema modules.
pub struct Generated {
    pub tokens: TokenStream,
    pub shared: Vec<SharedEnum>,
}

/// The items of a statement's module, from its analysis (the engine's `Analysis`, as JSON). An
/// error stands at `span`: the macro's statement, or the call site of a generator.
pub fn items(dialect: Dialect, options: Options, analysis: &Value, span: Span) -> syn::Result<TokenStream> {
    Ok(module(dialect, options, analysis, span, &Overrides::default(), false)?.tokens)
}

/// The items of a statement's module, with the crate's own items standing in for some of them; with
/// `shared`, each Postgres enum type is read from `__reylo_shared_root::<schema>`, which the
/// generator that writes the file points at its schema modules.
pub fn module(dialect: Dialect, options: Options, analysis: &Value, span: Span, overrides: &Overrides, shared: bool) -> syn::Result<Generated> {
    if let Some(diagnostics) = analysis["diagnostics"].as_array().filter(|d| !d.is_empty()) {
        let messages: Vec<&str> = diagnostics.iter().filter_map(|d| d["message"].as_str()).collect();
        return Err(syn::Error::new(span, messages.join("\n")));
    }
    if analysis["style"].as_str() == Some("positional") {
        return Err(syn::Error::new(span, "name each parameter, `:id`: it becomes a field of `Query`"));
    }
    let read = |key: &str| syn::Error::new(span, format!("the analysis's {key} could not be read: build again where the database can be reached"));
    let columns: Vec<Column> = serde_json::from_value(analysis["columns"].clone()).map_err(|_| read("columns"))?;
    let params: Vec<Param> = serde_json::from_value(analysis["params"].clone()).map_err(|_| read("params"))?;
    let domains = serde_json::from_value(analysis.get("domains").cloned().unwrap_or(Value::Null)).unwrap_or_default();
    // `/// One user, by id.`: rustdoc reads each line after the space a written comment leaves.
    let doc = analysis["comment"].as_str().map(|c| format!(" {}", c.replace('\n', "\n "))).map(|c| quote!(#[doc = #c]));
    let mut types = Types::new(dialect, options, domains, span, shared);
    let mut conversions = Conversions::default();

    // The row: one field per column, typed by its descriptor or by the crate's own type for it.
    let row_partial = match &overrides.row {
        Some(RowOverride::Partial(p)) => {
            check_names(p, columns.iter().map(|c| c.name.as_str()), "column", "a column of the statement")?;
            Some(p)
        }
        _ => None,
    };
    let whole_row = matches!(overrides.row, Some(RowOverride::Whole));
    if overrides.row.is_some() && columns.is_empty() {
        return Err(syn::Error::new(span, "the statement returns no rows, so its module has no `Row` for the crate's own to stand in for"));
    }
    let mut fields = Vec::new();
    let mut gets = Vec::new();
    let mut values = Vec::new();
    for (i, column) in columns.iter().enumerate() {
        let field = field_ident(&column.name, span)?;
        let shape = types.column(&column.ty, column.nullable, &column.name)?;
        let decode = shape.decode_ty();
        let index = proc_macro2::Literal::usize_unsuffixed(i);
        let local = format_ident!("__reylo_{}", i);
        gets.push(quote!(let #local = ::reylo::sqlx::Row::try_get::<#decode, _>(row, #index)?;));
        if !shape.decoded_as_is() {
            let decoded = shape.decode(quote!(#local), &column.name);
            gets.push(quote!(let #local = #decoded;));
        }
        let own = row_partial.and_then(|p| p.fields.iter().find(|f| f.name == column.name));
        let (ty, value) = if whole_row {
            // The crate's struct: an enum through `reylo::Enum::cast`, whose type the field gives.
            let value = match shape.enumeration() {
                Some(_) => shape.map_leaves(quote!(#local), &|v| quote!(::reylo::Enum::cast(#v))),
                None => quote!(::core::convert::Into::into(#local)),
            };
            (shape.ty(), value)
        } else if let Some(own) = own {
            let written = &own.ty;
            let value = match shape.enumeration() {
                Some(e) => {
                    let into = conversions.between(e, &shape.leaf_of(written, &column.name)?)?.0;
                    shape.map_leaves(quote!(#local), &|v| quote!(#into(#v)))
                }
                None => quote!(::core::convert::Into::into(#local)),
            };
            (quote!(#written), value)
        } else {
            (shape.ty(), quote!(#local))
        };
        let (attrs, vis) = match own {
            Some(own) => (own.attrs.clone(), own.vis.clone()),
            None => (Vec::new(), syn::parse_quote!(pub)),
        };
        fields.push(quote!(#(#attrs)* #vis #field: #ty));
        values.push(quote!(#field: #value));
    }

    // The parameters, one field each, in the order the statement first names them; text is borrowed.
    let params_partial = overrides.params.as_ref();
    if let Some(p) = params_partial {
        check_names(p, params.iter().map(|p| p.name.as_str()), "parameter", "a parameter of the statement")?;
    }
    let mut param_fields = Vec::new();
    let mut encodes = std::collections::HashMap::new();
    let mut borrows = false;
    for param in &params {
        let field = field_ident(&param.name, span)?;
        let (shape, borrowed) = types.param(&param.ty, param.nullable, &param.name)?;
        let own = params_partial.and_then(|p| p.fields.iter().find(|f| f.name == param.name));
        let this = quote!(&self.#field);
        let (ty, encode) = match own {
            Some(own) => {
                let written = &own.ty;
                let encode = match shape.enumeration() {
                    Some(e) => {
                        let from = conversions.between(e, &shape.leaf_of(written, &param.name)?)?.1;
                        shape.encode_leaves(this, &|leaf, ty, v| match leaf {
                            types::Leaf::Label => quote!(<#ty as ::reylo::Enum>::label(#from(#v))),
                            _ => quote!(#from(#v)),
                        })
                    }
                    None => quote!(::core::clone::Clone::clone(#this)),
                };
                (quote!(#written), encode)
            }
            None => {
                borrows |= borrowed;
                (shape.ty(), shape.encode(this))
            }
        };
        let (attrs, vis) = match own {
            Some(own) => (own.attrs.clone(), own.vis.clone()),
            None => (Vec::new(), syn::parse_quote!(pub)),
        };
        param_fields.push(quote!(#(#attrs)* #vis #field: #ty));
        encodes.insert(param.name.clone(), encode);
    }
    // Bound in the order the rewritten statement numbers them, a parameter written twice bound twice.
    let order: Vec<String> = analysis["rewritten"]["order"].as_array().cloned().unwrap_or_default().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    let mut binds = Vec::new();
    for name in &order {
        let encode = encodes.get(name).cloned().unwrap_or_else(|| {
            let field = field_ident(name, span).map(|f| quote!(#f)).unwrap_or_default();
            quote!(::core::clone::Clone::clone(&self.#field))
        });
        binds.push(quote!(.bind(#encode)));
    }

    let (db, db_row) = match dialect {
        Dialect::Postgres => (quote!(::reylo::sqlx::Postgres), quote!(::reylo::sqlx::postgres::PgRow)),
        Dialect::Sqlite => (quote!(::reylo::sqlx::Sqlite), quote!(::reylo::sqlx::sqlite::SqliteRow)),
        Dialect::Mysql => (quote!(::reylo::sqlx::MySql), quote!(::reylo::sqlx::mysql::MySqlRow)),
    };
    let text = analysis["rewritten"]["sql"].as_str().unwrap_or_default().to_string();
    let query = quote!(::reylo::sqlx::query::<#db>(SQL) #(#binds)*);
    let cardinality = analysis["cardinality"].as_str();
    // sqlx 0.8 binds a borrowed value for the life of the query, so a stream of rows lives no longer
    // than the text it was given.
    let outlives = borrows.then(|| quote!('a: 'e,));
    let (out, run, stream) = if columns.is_empty() {
        (quote!(u64), quote!(::core::result::Result::Ok(#query.execute(db).await?.rows_affected())), quote!())
    } else {
        match cardinality {
            Some("one") => (quote!(Row), quote!(__reylo_decode(&::reylo::__private::exactly_one(#query.fetch(db), SQL).await?)), quote!()),
            Some("at_most_one") => (
                quote!(::core::option::Option<Row>),
                quote!(match ::reylo::__private::at_most_one(#query.fetch(db), SQL).await? {
                    ::core::option::Option::Some(row) => ::core::result::Result::Ok(::core::option::Option::Some(__reylo_decode(&row)?)),
                    ::core::option::Option::None => ::core::result::Result::Ok(::core::option::Option::None),
                }),
                quote!(),
            ),
            _ => (
                quote!(::std::vec::Vec<Row>),
                quote!(#query.fetch_all(db).await?.iter().map(__reylo_decode).collect()),
                quote! {
                    /// Runs the statement and hands its rows on one at a time, as they arrive.
                    #[allow(dead_code)]
                    pub fn stream<'e, 'c: 'e, E>(&self, db: E) -> ::reylo::__private::BoxStream<'e, ::reylo::sqlx::Result<Row>>
                    where
                        #outlives
                        E: 'e + ::reylo::sqlx::Executor<'c, Database = #db>,
                    {
                        ::reylo::__private::rows(#query.fetch(db), __reylo_decode)
                    }
                },
            ),
        }
    };
    let decode = (!columns.is_empty()).then(|| {
        quote! {
            #[allow(clippy::useless_conversion)]
            fn __reylo_decode(row: &#db_row) -> ::core::result::Result<Row, ::reylo::sqlx::Error> {
                #(#gets)*
                ::core::result::Result::Ok(Row { #(#values,)* })
            }
        }
    });

    let row = match (&overrides.row, columns.is_empty()) {
        (_, true) | (Some(RowOverride::Whole), _) => quote!(),
        (Some(RowOverride::Partial(p)), false) => {
            let (attrs, vis) = (&p.attrs, &p.vis);
            quote!(#(#attrs)* #vis struct Row { #(#fields,)* })
        }
        (None, false) => {
            let serialize = types.serialize.then(|| quote!(#[derive(::reylo::serde::Serialize)] #[serde(crate = "::reylo::serde")]));
            quote! {
                /// One row of the statement's result.
                #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq)]
                #serialize
                pub struct Row { #(#fields,)* }
            }
        }
    };
    let lifetime = borrows.then(|| quote!(<'a>));
    let params_decl = match params_partial {
        Some(p) => {
            let (attrs, vis) = (&p.attrs, &p.vis);
            if param_fields.is_empty() {
                quote!(#(#attrs)* #vis struct Params;)
            } else {
                quote!(#(#attrs)* #vis struct Params #lifetime { #(#param_fields,)* })
            }
        }
        None if param_fields.is_empty() => quote!(#[derive(::core::fmt::Debug, ::core::clone::Clone)] pub struct Params;),
        None => quote!(#[derive(::core::fmt::Debug, ::core::clone::Clone)] pub struct Params #lifetime { #(#param_fields,)* }),
    };
    let finished = types.finish(&overrides.taken)?;
    let nested = &finished.items;
    let converts = &conversions.tokens;
    let tokens = quote! {
        #(#nested)*
        #(#converts)*
        /// The statement as it is sent, its parameters written as the database's own placeholders.
        pub const SQL: &str = #text;
        #doc
        #params_decl
        /// The query, built from its parameters and run with `run`: another name for `Params`,
        /// so that `get_user::Query { id: 2 }.run(&pool)` reads as the query it runs.
        pub use Params as Query;
        #row
        #decode
        impl #lifetime Params #lifetime {
            /// Runs the statement on a pool, a connection or a transaction.
            pub async fn run<'e, E>(&self, db: E) -> ::reylo::sqlx::Result<#out>
            where
                E: ::reylo::sqlx::Executor<'e, Database = #db>,
            {
                #run
            }
            #stream
        }
    };
    Ok(Generated { tokens: finished.named(tokens), shared: finished.shared })
}

/// The functions between the module's enums and the crate's own: one each way per pair, each an
/// exhaustive `match` on the variants' names, so a variant the crate's enum lacks, or one it has
/// beyond the database's values, fails to compile.
#[derive(Default)]
struct Conversions {
    tokens: Vec<TokenStream>,
    made: Vec<(String, String, Ident, Ident)>,
}

impl Conversions {
    /// The functions from the module's enum `e` to `written`, the crate's, and back.
    fn between(&mut self, e: &types::EnumRef, written: &syn::Type) -> syn::Result<(Ident, Ident)> {
        let key = (e.ident.to_string(), quote!(#written).to_string());
        if let Some((_, _, into, from)) = self.made.iter().find(|(a, b, _, _)| (a, b) == (&key.0, &key.1)) {
            return Ok((into.clone(), from.clone()));
        }
        let syn::Type::Path(path) = written else {
            return Err(syn::Error::new_spanned(written, "the field holds a database enum: name the crate's own enum, as `crate::jobs::JobStatus`"));
        };
        if path.qself.is_some() {
            return Err(syn::Error::new_spanned(written, "the field holds a database enum: name the crate's own enum by its path, as `crate::jobs::JobStatus`"));
        }
        let n = self.made.len();
        let (into, from) = (format_ident!("__reylo_into_{}", n), format_ident!("__reylo_from_{}", n));
        let ours = &e.ident;
        let variants: Vec<Ident> = names::variants(&e.labels).iter().map(|v| format_ident!("{}", v)).collect();
        let span = syn::spanned::Spanned::span(written);
        let theirs: Vec<TokenStream> = variants.iter().map(|v| quote_spanned!(span=> #path::#v)).collect();
        self.tokens.push(quote! {
            #[allow(dead_code)]
            fn #into(value: #ours) -> #written {
                match value { #(#ours::#variants => #theirs,)* }
            }
            #[allow(dead_code)]
            fn #from(value: &#written) -> #ours {
                match value { #(#theirs => #ours::#variants,)* }
            }
        });
        self.made.push((key.0, key.1, into.clone(), from.clone()));
        Ok((into, from))
    }
}

/// An error at each field of `own` that names nothing the statement has.
fn check_names<'a>(own: &Partial, known: impl Iterator<Item = &'a str>, what: &str, hint: &str) -> syn::Result<()> {
    let known: Vec<&str> = known.collect();
    let mut errors: Option<syn::Error> = None;
    for field in &own.fields {
        if !known.contains(&field.name.as_str()) {
            let listed = if known.is_empty() { format!("the statement has no {what}") } else { format!("its {what}s are {}", known.join(", ")) };
            let error = syn::Error::new_spanned(&field.ty, format!("`{}` is not {hint}: {listed}", field.name));
            match &mut errors {
                Some(e) => e.combine(error),
                None => errors = Some(error),
            }
        }
    }
    errors.map_or(Ok(()), Err)
}

/// The stand-ins for a statement that cannot be typed, so that items written beside them add no
/// errors of their own: none where the crate's own item stands in for one.
pub fn stand_ins(overrides: &Overrides) -> TokenStream {
    let row = (!matches!(overrides.row, Some(RowOverride::Whole))).then(|| quote!(pub struct Row {}));
    quote!(pub struct Params {} pub use Params as Query; #row)
}

/// A column's or a parameter's name as a Rust field.
pub fn field_ident(name: &str, span: Span) -> syn::Result<Ident> {
    // `self`, `Self`, `super`, `crate` and `_` are keywords no raw identifier can be.
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || name.starts_with(|c: char| c.is_ascii_digit()) || matches!(name, "self" | "Self" | "super" | "crate" | "_") {
        return Err(syn::Error::new(span, format!("the column {name:?} is no Rust name: give it one with `as`")));
    }
    Ok(syn::parse_str::<Ident>(name).unwrap_or_else(|_| Ident::new_raw(name, span)))
}

#[cfg(test)]
mod tests {
    use super::field_ident;
    use proc_macro2::Span;

    #[test]
    fn a_keyword_column_is_an_error_not_a_panic() {
        for name in ["self", "Self", "super", "crate", "_", "", "1st", "a-b"] {
            assert!(field_ident(name, Span::call_site()).is_err(), "{name}");
        }
        assert_eq!(field_ident("type", Span::call_site()).unwrap().to_string(), "r#type");
        assert_eq!(field_ident("name", Span::call_site()).unwrap().to_string(), "name");
    }
}
