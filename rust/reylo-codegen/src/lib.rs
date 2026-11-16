//! A statement's analysis as the Rust items of its module, which `#[reylo::query]` expands to and a
//! generator writes to a file:
//!
//! - `Params`, a struct of the statement's parameters, one field per `:name` (a text parameter a
//!   `&str`), whose `run(executor)` runs the statement on sqlx and returns what the cardinality
//!   says: `Row` for one row, `Option<Row>` for at most one, `Vec<Row>` for many, and the affected
//!   row count for a write that returns none; and `Query`, the same struct by another name, so a
//!   call reads `get_user::Query { id: 2 }.run(&pool)`;
//! - `Row`, one row, each field typed from the column's type descriptor (`spec/`): a nullable
//!   column `Option<T>`, an array whose elements may be NULL `Vec<Option<T>>`, a
//!   `json_build_object` a struct of its fields; none for a write that returns no rows;
//! - an enum per Postgres enum type the statement reads or binds, and a struct per JSON object it
//!   builds.
//!
//! Every path the items name is written in full (`::core::option::Option`, `::reylo::sqlx`), since
//! the macro puts them beside the crate's own items, which may shadow a prelude name.

mod types;

/// A statement's analysis, from the project's database or `reylo/.cache/`.
#[cfg(feature = "analyze")]
pub mod analysis;
/// The `.sql` files a path, a directory or a glob names.
#[cfg(feature = "generate")]
pub mod files;
/// The modules written to files.
#[cfg(feature = "generate")]
pub mod generate;

/// The items as a file shows them, the prelude's names written short.
pub mod readable;

pub use types::Types;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use reylo_engine_core::api::{Column, Param};
use serde_json::Value;
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

/// The items of a statement's module, from its analysis (the engine's `Analysis`, as JSON). An
/// error stands at `span`: the macro's statement, or the call site of a generator.
pub fn items(dialect: Dialect, options: Options, analysis: &Value, span: Span) -> syn::Result<TokenStream> {
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
    let mut types = Types::new(dialect, options, domains, span);

    // The row: one field per column, typed by its descriptor.
    let mut fields = Vec::new();
    let mut gets = Vec::new();
    for (i, column) in columns.iter().enumerate() {
        let field = field_ident(&column.name, span)?;
        let shape = types.column(&column.ty, column.nullable, &column.name)?;
        let ty = shape.ty();
        let decode = shape.decode_ty();
        let index = proc_macro2::Literal::usize_unsuffixed(i);
        let value = shape.unwrap(quote!(::reylo::sqlx::Row::try_get::<#decode, _>(row, #index)?));
        fields.push(quote!(pub #field: #ty));
        gets.push(quote!(#field: #value));
    }

    // The parameters, one field each, in the order the statement first names them; text is borrowed.
    let mut param_fields = Vec::new();
    let mut borrows = false;
    for param in &params {
        let field = field_ident(&param.name, span)?;
        let ty = if types.is_string(&param.ty) {
            borrows = true;
            quote!(&'a str)
        } else {
            types.param(&param.ty, &param.name)?
        };
        let ty = if param.nullable { quote!(::core::option::Option<#ty>) } else { ty };
        param_fields.push(quote!(pub #field: #ty));
    }
    // Bound in the order the rewritten statement numbers them, a parameter written twice bound twice.
    let order: Vec<String> = analysis["rewritten"]["order"].as_array().cloned().unwrap_or_default().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    let mut binds = Vec::new();
    for name in &order {
        let field = field_ident(name, span)?;
        binds.push(quote!(.bind(::core::clone::Clone::clone(&self.#field))));
    }

    let (db, db_row) = match dialect {
        Dialect::Postgres => (quote!(::reylo::sqlx::Postgres), quote!(::reylo::sqlx::postgres::PgRow)),
        Dialect::Sqlite => (quote!(::reylo::sqlx::Sqlite), quote!(::reylo::sqlx::sqlite::SqliteRow)),
        Dialect::Mysql => (quote!(::reylo::sqlx::MySql), quote!(::reylo::sqlx::mysql::MySqlRow)),
    };
    let text = analysis["rewritten"]["sql"].as_str().unwrap_or_default().to_string();
    let query = quote!(::reylo::sqlx::query::<#db>(#text) #(#binds)*);
    let cardinality = analysis["cardinality"].as_str();
    let (out, run) = if columns.is_empty() {
        (quote!(u64), quote!(::core::result::Result::Ok(#query.execute(db).await?.rows_affected())))
    } else {
        let (out, fetch) = match cardinality {
            Some("one") => (quote!(Row), quote!(decode(&#query.fetch_one(db).await?))),
            Some("at_most_one") => (
                quote!(::core::option::Option<Row>),
                quote!(match #query.fetch_optional(db).await? {
                    ::core::option::Option::Some(row) => ::core::result::Result::Ok(::core::option::Option::Some(decode(&row)?)),
                    ::core::option::Option::None => ::core::result::Result::Ok(::core::option::Option::None),
                }),
            ),
            _ => (quote!(::std::vec::Vec<Row>), quote!(#query.fetch_all(db).await?.iter().map(decode).collect())),
        };
        let run = quote! {
            fn decode(row: &#db_row) -> ::core::result::Result<Row, ::reylo::sqlx::Error> {
                ::core::result::Result::Ok(Row { #(#gets,)* })
            }
            #fetch
        };
        (out, run)
    };

    let row = if columns.is_empty() {
        quote!()
    } else {
        let serialize = types.serialize.then(|| quote!(#[derive(::reylo::serde::Serialize)] #[serde(crate = "::reylo::serde")]));
        quote! {
            /// One row of the statement's result.
            #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq)]
            #serialize
            pub struct Row { #(#fields,)* }
        }
    };
    let lifetime = borrows.then(|| quote!(<'a>));
    let params_decl = if param_fields.is_empty() { quote!(pub struct Params;) } else { quote!(pub struct Params #lifetime { #(#param_fields,)* }) };
    let nested = types.items();
    Ok(quote! {
        #(#nested)*
        #doc
        #[derive(::core::fmt::Debug, ::core::clone::Clone)]
        #params_decl
        /// The query, built from its parameters and run with `run`: another name for `Params`,
        /// so that `get_user::Query { id: 2 }.run(&pool)` reads as the query it runs.
        pub use Params as Query;
        #row
        impl #lifetime Params #lifetime {
            /// Runs the statement on a pool, a connection or a transaction.
            pub async fn run<'e, E>(&self, db: E) -> ::reylo::sqlx::Result<#out>
            where
                E: ::reylo::sqlx::Executor<'e, Database = #db>,
            {
                #run
            }
        }
    })
}

/// The stand-ins for a statement that cannot be typed, so that items written beside them add no
/// errors of their own.
pub fn stand_ins() -> TokenStream {
    quote!(pub struct Params {} pub use Params as Query; pub struct Row {})
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
