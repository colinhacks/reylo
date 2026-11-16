//! The attribute macro. `#[reylo::query("… where id = :id")] pub mod get_user {}` analyzes the
//! statement with Reylo's engine at compile time and fills the module with:
//!
//! - `Params`, a struct of the statement's parameters, one field per `:name` (a text parameter a
//!   `&str`), whose `run(executor)` runs the statement on sqlx and returns what the cardinality
//!   says: `Row` for one row, `Option<Row>` for at most one, `Vec<Row>` for many, and the affected
//!   row count for a write that returns none, and `Query`, the same struct by another name, so a
//!   call reads `get_user::Query { id: 2 }.run(&pool)`;
//! - `Row`, one row, each field typed from the column's type descriptor (`spec/`): a nullable
//!   column `Option<T>`, an array whose elements may be NULL `Vec<Option<T>>`, a
//!   `json_build_object` a struct of its fields; none for a write that returns no rows;
//! - an enum per Postgres enum type the statement reads or binds, and a struct per JSON object it
//!   builds.
//!
//! Items the crate writes inside the module's braces are kept, so `impl Row { … }` can sit with the
//! query; the module sees the items of the one around it. A value a type cannot hold is refused
//! when it is read (`reylo::Exact`); where no Rust type holds a column's values, the expansion fails
//! at the statement and says how to cast it.
//!
//! The run time is sqlx's: the query runs on the caller's sqlx pool, connection or transaction.
//! Postgres, SQLite and MySQL, whichever the URL's scheme names.

use reylo_codegen::{Dates, Decimals, Options};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemMod, LitStr};

/// A statement, typed by the database at compile time: `#[reylo::query("select …")] pub mod get_user {}`.
#[proc_macro_attribute]
pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    let sql = syn::parse_macro_input!(attr as LitStr);
    let module = syn::parse_macro_input!(item as ItemMod);
    expand(&sql, module).into()
}

fn expand(sql: &LitStr, module: ItemMod) -> TokenStream2 {
    let ItemMod { attrs, vis, ident, content, .. } = module;
    let Some((_, own)) = content else {
        return syn::Error::new(ident.span(), format!("write the module with braces, `mod {ident} {{}}`: the query's types go inside it")).to_compile_error();
    };
    // Where the statement cannot be typed, its error stands at the statement, and empty stand-ins
    // for the two structs keep the crate's own items in the module from adding errors of their own.
    let body = items(sql).unwrap_or_else(|e| {
        let error = e.to_compile_error();
        let stand_ins = reylo_codegen::stand_ins();
        quote!(#error #stand_ins)
    });
    quote! {
        #(#attrs)*
        #vis mod #ident {
            #[allow(unused_imports)]
            use super::*;
            #body
            #(#own)*
        }
    }
}

fn items(sql: &LitStr) -> syn::Result<TokenStream2> {
    let span = sql.span();
    let manifest = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| syn::Error::new(span, "CARGO_MANIFEST_DIR is not set"))?;
    let (dialect, analysis) = reylo_codegen::analysis::analyze(std::path::Path::new(&manifest), &sql.value()).map_err(|message| syn::Error::new(span, message))?;
    reylo_codegen::items(dialect, options(), &analysis, span)
}

/// The crates the expansion may name, as `reylo`'s features of the same names turn them on.
fn options() -> Options {
    Options {
        dates: if cfg!(feature = "chrono") {
            Some(Dates::Chrono)
        } else if cfg!(feature = "time") {
            Some(Dates::Time)
        } else {
            None
        },
        uuid: cfg!(feature = "uuid"),
        decimals: if cfg!(feature = "rust_decimal") {
            Some(Decimals::RustDecimal)
        } else if cfg!(feature = "bigdecimal") {
            Some(Decimals::BigDecimal)
        } else {
            None
        },
    }
}
