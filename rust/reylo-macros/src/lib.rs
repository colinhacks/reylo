//! The attribute macros. `#[reylo::query("… where id = :id")] pub mod get_user {}` analyzes the
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
//! - `SQL`, the statement as it is sent;
//! - an enum per database enum the statement reads or binds, and a struct per JSON object it
//!   builds.
//!
//! Items the crate writes inside the module's braces are kept, so `impl Row { … }` can sit with the
//! query; the module sees the items of the one around it. Three of them stand in for generated
//! ones: `pub use crate::jobs::QueryJob as Row;` makes the crate's struct the row, and a
//! `pub struct Row { … }` or `pub struct Params { … }` with some of the fields gives those fields
//! the crate's types, as an enum of its own for a column of a database enum. A value a type cannot
//! hold is refused when it is read (`reylo::Exact`); where no Rust type holds a column's values,
//! the expansion fails at the statement and says how to cast it. A statement the database refuses
//! fails at its string with the database's error, and the file, line and column inside the string
//! it points at.
//!
//! `#[reylo::reflect("public.job_kind")]` on an enum of the crate's own checks its variants
//! against the database enum's values, or fills an empty one, and makes it a `reylo::Enum`.
//!
//! The run time is sqlx's: the query runs on the caller's sqlx pool, connection or transaction.
//! Postgres, SQLite and MySQL, whichever the URL's scheme names.

use reylo_build::{Dates, Decimals, Dialect, Options, Overrides, Partial, PartialField, RowOverride};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{Item, ItemEnum, ItemMod, LitStr, UseTree};

/// A statement, typed by the database at compile time: `#[reylo::query("select …")] pub mod get_user {}`.
#[proc_macro_attribute]
pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    let sql = syn::parse_macro_input!(attr as LitStr);
    let module = syn::parse_macro_input!(item as ItemMod);
    expand(&sql, module).into()
}

fn expand(sql: &LitStr, module: ItemMod) -> TokenStream2 {
    let ItemMod { attrs, vis, ident, content, .. } = module;
    let Some((_, mut own)) = content else {
        return syn::Error::new(ident.span(), format!("write the module with braces, `mod {ident} {{}}`: the query's types go inside it")).to_compile_error();
    };
    let overrides = match overrides(&mut own) {
        Ok(overrides) => overrides,
        Err(e) => {
            let error = e.to_compile_error();
            return quote!(#(#attrs)* #vis mod #ident { #error });
        }
    };
    // Where the statement cannot be typed, its error stands at the statement, and empty stand-ins
    // for the two structs keep the crate's own items in the module from adding errors of their own.
    let body = items(sql, &overrides).unwrap_or_else(|e| {
        let error = e.to_compile_error();
        let stand_ins = reylo_build::stand_ins(&overrides);
        quote!(#error #stand_ins)
    });
    let tracked = tracked();
    quote! {
        #(#attrs)*
        #vis mod #ident {
            #[allow(unused_imports)]
            use super::*;
            #(const _: &str = ::core::include_str!(#tracked);)*
            #body
            #(#own)*
        }
    }
}

/// The project's migrations, its lock and its `schema.sql`, compiled into the crate:
/// `reylo::migrate!().run(&pool)` runs `reylo migrate deploy` with them, and `.status(&pool)`
/// `reylo migrate status`. Each file is included as it stands at the build, so Cargo compiles the
/// crate again when one changes, the lock among them, which `reylo migrate dev` writes when it adds
/// a migration. Cargo watches no directory for a macro, so a file added where no lock lists the
/// migrations, or a lock or `schema.sql` made after the build, needs the build script line the
/// docs give (`cargo:rerun-if-changed=reylo/migrations`).
#[proc_macro]
pub fn migrate(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        let input = TokenStream2::from(input);
        return syn::Error::new_spanned(input, "reylo::migrate!() takes nothing: it reads the migrations of the project's reylo/ directory, or of the one .reylo.yaml's `dir` names").to_compile_error().into();
    }
    migrations().unwrap_or_else(|e| e.to_compile_error()).into()
}

fn migrations() -> syn::Result<TokenStream2> {
    let span = proc_macro2::Span::call_site();
    let manifest = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| syn::Error::new(span, "CARGO_MANIFEST_DIR is not set"))?;
    let (root, home) = reylo_build::analysis::home(std::path::Path::new(&manifest)).map_err(|e| syn::Error::new(span, e))?;
    // As messages show the project's paths, `reylo/migrations` on every platform.
    let shown = home.strip_prefix(&root).unwrap_or(&home).to_string_lossy().replace('\\', "/");
    let shown = if shown.is_empty() { ".".to_string() } else { shown };
    let dir = home.join("migrations");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Err(syn::Error::new(span, format!("there is no {}/migrations directory ({}): reylo migrate init makes one", shown, dir.display())));
    };
    let mut files: Vec<(String, std::path::PathBuf)> = entries.filter_map(Result::ok).map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())).filter(|(name, _)| !name.starts_with('.')).collect();
    files.sort();
    // Each migration with its text; any other entry, a directory among them, by its name alone,
    // which deploy and status name as a stray, as they do on the command line.
    let files = files.iter().map(|(name, path)| {
        if name.ends_with(".sql") && path.is_file() {
            let path = path.to_string_lossy();
            quote!((#name, ::core::include_str!(#path)))
        } else {
            quote!((#name, ""))
        }
    });
    let included = |path: std::path::PathBuf| {
        if path.is_file() {
            let path = path.to_string_lossy().into_owned();
            quote!(::core::option::Option::Some(::core::include_str!(#path)))
        } else {
            quote!(::core::option::Option::None)
        }
    };
    let lock = included(home.join("reylo-lock.yaml"));
    let schema = included(home.join("schema.sql"));
    Ok(quote!(::reylo::migrate::Migrator::__new(#shown, &[#(#files),*], #lock, #schema)))
}

/// The files a statement's types depend on beside it: the project's `schema.sql` and lock, and its
/// project file. Each is included in the expansion, which has cargo compile the crate again when
/// one changes, so a statement is typed against the schema a `reylo db push` or `reylo migrate dev`
/// just made rather than read from the last build. A `.env` file is left out, since an expansion
/// shown in an editor would show what it holds.
fn tracked() -> Vec<LitStr> {
    let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") else { return Vec::new() };
    reylo_build::analysis::watched(std::path::Path::new(&manifest))
        .into_iter()
        .filter(|path| path.is_file() && !path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(".env")))
        .map(|path| LitStr::new(&path.to_string_lossy(), proc_macro2::Span::call_site()))
        .collect()
}

/// What the module's own items stand in for, taken out of `own` where the generated item replaces
/// them: a partial `Row` or `Params` struct. The names of the rest are left to them.
fn overrides(own: &mut Vec<Item>) -> syn::Result<Overrides> {
    let mut found = Overrides::default();
    let mut kept = Vec::new();
    for item in std::mem::take(own) {
        match &item {
            Item::Struct(s) if s.ident == "Row" || s.ident == "Params" => {
                if !s.generics.params.is_empty() {
                    return Err(syn::Error::new_spanned(&s.generics, format!("write `{}` without generics: the macro gives it the lifetime a text parameter needs", s.ident)));
                }
                let fields = match &s.fields {
                    syn::Fields::Named(named) => named.named.iter().map(|f| PartialField { name: f.ident.as_ref().map(|i| i.unraw().to_string()).unwrap_or_default(), attrs: f.attrs.clone(), vis: f.vis.clone(), ty: f.ty.clone() }).collect(),
                    syn::Fields::Unit => Vec::new(),
                    syn::Fields::Unnamed(_) => return Err(syn::Error::new_spanned(&s.fields, format!("name `{}`'s fields after the columns or parameters they stand for", s.ident))),
                };
                let partial = Partial { attrs: s.attrs.clone(), vis: s.vis.clone(), fields, span: s.ident.span() };
                if s.ident == "Row" {
                    found.row = Some(RowOverride::Partial(partial));
                } else {
                    found.params = Some(partial);
                }
                continue;
            }
            Item::Use(u) => {
                let mut names = Vec::new();
                use_names(&u.tree, &mut names);
                for name in names {
                    claim(&mut found, &name, &item)?;
                }
            }
            other => {
                if let Some(name) = item_name(other) {
                    claim(&mut found, &name, &item)?;
                }
            }
        }
        kept.push(item);
    }
    *own = kept;
    Ok(found)
}

/// Records that the module's own item `item` takes `name`: `Row` makes the crate's struct the row.
fn claim(found: &mut Overrides, name: &str, item: &Item) -> syn::Result<()> {
    match name {
        "Row" => match item {
            Item::Use(_) | Item::Type(_) => found.row = Some(RowOverride::Whole),
            _ => return Err(syn::Error::new_spanned(item, "a `Row` of the crate's own is a struct with some of the row's fields, `pub struct Row { … }`, or one named elsewhere, `pub use crate::jobs::QueryJob as Row;`")),
        },
        "Params" | "Query" => return Err(syn::Error::new_spanned(item, format!("`{name}` is the query's own: give some of its fields the crate's types with `pub struct Params {{ … }}`, holding just those fields"))),
        _ => {
            found.taken.insert(name.to_string());
        }
    }
    Ok(())
}

/// The names a `use` brings into the module; none for a glob.
fn use_names(tree: &UseTree, out: &mut Vec<String>) {
    match tree {
        UseTree::Path(p) => use_names(&p.tree, out),
        UseTree::Name(n) => out.push(n.ident.unraw().to_string()),
        UseTree::Rename(r) => out.push(r.rename.unraw().to_string()),
        UseTree::Group(g) => g.items.iter().for_each(|t| use_names(t, out)),
        UseTree::Glob(_) => {}
    }
}

/// The name an item declares in the module.
fn item_name(item: &Item) -> Option<String> {
    let ident = match item {
        Item::Struct(i) => &i.ident,
        Item::Enum(i) => &i.ident,
        Item::Type(i) => &i.ident,
        Item::Union(i) => &i.ident,
        Item::Trait(i) => &i.ident,
        Item::Mod(i) => &i.ident,
        Item::Fn(i) => &i.sig.ident,
        Item::Const(i) => &i.ident,
        Item::Static(i) => &i.ident,
        _ => return None,
    };
    Some(ident.unraw().to_string())
}

fn items(sql: &LitStr, overrides: &Overrides) -> syn::Result<TokenStream2> {
    let span = sql.span();
    let (dialect, analysis) = analyze(sql, &sql.value())?;
    Ok(reylo_build::module(dialect, options(), &analysis, span, overrides, false)?.tokens)
}

/// The analysis of `text`, which the literal `at` holds: the engine's diagnostics as errors at the
/// literal, each placed inside it.
fn analyze(at: &LitStr, text: &str) -> syn::Result<(Dialect, serde_json::Value)> {
    let span = at.span();
    let manifest = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| syn::Error::new(span, "CARGO_MANIFEST_DIR is not set"))?;
    let (dialect, analysis) = reylo_build::analysis::analyze(std::path::Path::new(&manifest), text).map_err(|message| syn::Error::new(span, message))?;
    if let Some(diagnostics) = analysis["diagnostics"].as_array().filter(|d| !d.is_empty()) {
        let each = diagnostics.iter().map(|d| (d["message"].as_str().unwrap_or("the statement cannot be analyzed"), d["start"].as_u64().map_or(0, |s| s as usize)));
        return Err(refused(at, each));
    }
    Ok((dialect, analysis))
}

/// An error at the statement for each of the engine's diagnostics. A compiler error cannot point
/// inside a string on stable Rust, so one the engine places past the statement's first byte also
/// names its file, line and column, as `at src/main.rs:3:16` under the message. Where the macro's
/// host cannot say where the string is (rust-analyzer's macro server, without a file to read, names
/// none), the error stands at the string alone.
fn refused<'a>(sql: &LitStr, diagnostics: impl Iterator<Item = (&'a str, usize)>) -> syn::Error {
    let start = sql.span().unwrap();
    let file = start.file();
    let literal = sql.token().to_string();
    let mut errors = diagnostics.map(|(message, offset)| {
        if offset == 0 || file.is_empty() {
            return syn::Error::new(sql.span(), message);
        }
        let (line, column) = reylo_build::place::in_literal(&literal, start.line(), start.column(), offset);
        syn::Error::new(sql.span(), format!("{message}\nat {file}:{line}:{column}"))
    });
    let mut error = errors.next().expect("a diagnostic");
    for next in errors {
        error.combine(next);
    }
    error
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

/// One of the crate's enums as a database enum: `#[reylo::reflect("public.job_kind")]` names a
/// Postgres enum type, and `#[reylo::reflect("events.kind")]` a column whose values a `CHECK`
/// constraint or a MySQL `ENUM` lists; the database says which. A name that is both is the type,
/// and the column is written with its schema (`public.events.kind`). The variants are the values'
/// names, as a query's module names them; an enum with none is given them all. It goes on an enum:
/// a struct would reflect a table's row, which it does not write.
#[proc_macro_attribute]
pub fn reflect(attr: TokenStream, item: TokenStream) -> TokenStream {
    // The item stays beside an error, so that its uses do not fail too.
    let name = match syn::parse::<LitStr>(attr) {
        Ok(name) => name,
        Err(e) => {
            let error = syn::Error::new(e.span(), "name the database enum, as `#[reylo::reflect(\"public.job_kind\")]`, or a column whose values a CHECK or an ENUM lists, as `#[reylo::reflect(\"events.kind\")]`").to_compile_error();
            let item = TokenStream2::from(item);
            return quote!(#item #error).into();
        }
    };
    let item = syn::parse_macro_input!(item as Item);
    let Item::Enum(item) = item else {
        let error = syn::Error::new_spanned(&item, "`#[reylo::reflect]` goes on an enum, whose variants are the database enum's values").to_compile_error();
        return quote!(#item #error).into();
    };
    match expand_enum(&name, item.clone()) {
        Ok(tokens) => tokens.into(),
        Err(e) => {
            let error = e.to_compile_error();
            quote!(#item #error).into()
        }
    }
}

/// What the database says of a statement written to read what a `reflect` names.
enum Lookup {
    /// A list of values.
    Found(Dialect, serde_json::Value),
    /// A value of another type, which the label names.
    Other(String),
    /// The database's errors, as for a type that does not exist.
    Refused(Vec<String>),
}

fn lookup(at: &LitStr, sql: &str) -> syn::Result<Lookup> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| syn::Error::new(at.span(), "CARGO_MANIFEST_DIR is not set"))?;
    let (dialect, analysis) = reylo_build::analysis::analyze(std::path::Path::new(&manifest), sql).map_err(|message| syn::Error::new(at.span(), message))?;
    if let Some(diagnostics) = analysis["diagnostics"].as_array().filter(|d| !d.is_empty()) {
        return Ok(Lookup::Refused(diagnostics.iter().map(|d| d["message"].as_str().unwrap_or("the statement cannot be analyzed").to_string()).collect()));
    }
    let ty = &analysis["columns"][0]["type"];
    if ty["kind"].as_str() == Some("enum") {
        return Ok(Lookup::Found(dialect, analysis));
    }
    Ok(Lookup::Other(ty["label"].as_str().or(ty["kind"].as_str()).unwrap_or_default().to_string()))
}

fn expand_enum(at: &LitStr, mut item: ItemEnum) -> syn::Result<TokenStream2> {
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&item.generics, "a database enum's Rust enum takes no generics"));
    }
    // The name as a type, then as a column; a type wins where the name is both.
    let path = at.value();
    let as_type = lookup(at, &format!("select cast(null as {path}) as value"));
    let as_column = path.rsplit_once('.').map(|(table, column)| lookup(at, &format!("select {column} as value from {table}")));
    let (dialect, analysis) = match (as_type, as_column) {
        (Ok(Lookup::Found(dialect, analysis)), _) | (_, Some(Ok(Lookup::Found(dialect, analysis)))) => (dialect, analysis),
        (Err(e), _) | (_, Some(Err(e))) => return Err(e),
        (Ok(as_type), as_column) => {
            // Neither is a list: what the database said of each, since either may be what was meant.
            let said = |what: &str, found: Lookup| match found {
                Lookup::Refused(messages) => format!("\nas {what}: {}", messages.join("; ")),
                Lookup::Other(label) if label.is_empty() => format!("\nas {what}: it lists no values"),
                Lookup::Other(label) => format!("\nas {what}: `{label}`, which lists no values"),
                Lookup::Found(..) => unreachable!(),
            };
            let mut message = format!("{path} is no list of values: name a Postgres enum type, or a column a CHECK (… in (…)) or a MySQL ENUM lists values for, with its table, as `events.kind`");
            message += &said("a type", as_type);
            if let Some(Ok(as_column)) = as_column {
                message += &said("a column", as_column);
            }
            return Err(syn::Error::new_spanned(at, message));
        }
    };
    let ty = &analysis["columns"][0]["type"];
    let values: Vec<String> = ty["values"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
    let label = ty["label"].as_str().unwrap_or_default();
    let domains = analysis.get("domains").and_then(|d| d.as_object());
    let is_type = dialect == Dialect::Postgres && label.contains('.') && !domains.is_some_and(|d| d.contains_key(label));
    let type_name = if is_type { label.to_string() } else { String::new() };

    // The variants: the values' names, the crate's own checked against them.
    let wanted = reylo_build::variants(&values);
    if let Some(v) = item.variants.iter().find(|v| !matches!(v.fields, syn::Fields::Unit)) {
        return Err(syn::Error::new_spanned(&v.fields, "a database enum's variants hold no fields"));
    }
    if item.variants.is_empty() {
        item.variants = wanted.iter().map(|w| -> syn::Variant { let ident = format_ident!("{}", w); syn::parse_quote!(#ident) }).collect();
    } else {
        let have: Vec<String> = item.variants.iter().map(|v| v.ident.to_string()).collect();
        let missing: Vec<&String> = wanted.iter().filter(|w| !have.contains(w)).collect();
        let extra: Vec<&syn::Variant> = item.variants.iter().filter(|v| !wanted.contains(&v.ident.to_string())).collect();
        if !missing.is_empty() || !extra.is_empty() {
            let pairs: Vec<String> = values.iter().zip(&wanted).map(|(v, w)| format!("{w} ({v:?})")).collect();
            let mut message = format!("{} has the values {}", at.value(), pairs.join(", "));
            if !missing.is_empty() {
                message.push_str(&format!("; add {}", missing.iter().map(|m| m.as_str()).collect::<Vec<_>>().join(", ")));
            }
            let mut error = syn::Error::new_spanned(&item.ident, message);
            for v in extra {
                error.combine(syn::Error::new_spanned(&v.ident, format!("`{}` is none of {}'s values", v.ident, at.value())));
            }
            return Err(error);
        }
    }
    // `reylo::Enum` is `Copy`, which a fieldless enum always can be.
    let derived: Vec<String> = item.attrs.iter().filter(|a| a.path().is_ident("derive")).flat_map(|a| a.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated).map(|p| p.into_iter().filter_map(|p| p.segments.last().map(|s| s.ident.to_string())).collect::<Vec<_>>()).unwrap_or_default()).collect();
    if !derived.iter().any(|d| d == "Clone") {
        item.attrs.push(syn::parse_quote!(#[derive(::core::clone::Clone)]));
    }
    if !derived.iter().any(|d| d == "Copy") {
        item.attrs.push(syn::parse_quote!(#[derive(::core::marker::Copy)]));
    }
    item.attrs.push(syn::parse_quote!(#[allow(non_camel_case_types)]));

    let ident = &item.ident;
    let trait_impl = reylo_build::enum_impl(ident, &type_name, &values);
    let sqlx = if is_type {
        reylo_build::postgres_impls(ident, &type_name)
    } else {
        let db = match dialect {
            Dialect::Postgres => quote!(Postgres),
            Dialect::Mysql => quote!(MySql),
            Dialect::Sqlite => quote!(Sqlite),
        };
        reylo_build::text_impls(ident, &db)
    };
    Ok(quote!(#item #trait_impl #sqlx))
}
