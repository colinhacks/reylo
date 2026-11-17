//! The Rust types of each statement the docs site's inference cards show, as the attribute writes
//! them: reads a line of JSON per statement, `{"key", "dialect", "analysis"}`, and writes a line of
//! JSON for each, `{"key", "params", "returns"}`, or `{"key", "error"}` with the message the
//! attribute fails with. `params` is the `Query` struct, `returns` what `.run()` returns, then
//! `Row` and the enums and structs it names, with the derives, the `run` method and the full paths
//! the expansion writes left out, as a reader writes them.

use proc_macro2::Span;
use quote::ToTokens;
use reylo_build::{Dates, Decimals, Dialect, Options};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn main() {
    let options = Options { dates: Some(Dates::Chrono), uuid: true, decimals: Some(Decimals::RustDecimal) };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("a line of standard input");
        if line.trim().is_empty() {
            continue;
        }
        let input: Value = serde_json::from_str(&line).expect("a line of JSON");
        let key = input["key"].clone();
        let dialect = match input["dialect"].as_str() {
            Some("postgres") => Dialect::Postgres,
            Some("mysql") => Dialect::Mysql,
            Some("sqlite") => Dialect::Sqlite,
            other => panic!("no dialect {other:?}"),
        };
        let answer = match reylo_build::items(dialect, options, &input["analysis"], Span::call_site()) {
            Ok(tokens) => {
                let (params, returns) = shown(syn::parse2(tokens).expect("the items parse"));
                json!({ "key": key, "params": params, "returns": returns })
            }
            Err(e) => json!({ "key": key, "error": e.to_string() }),
        };
        writeln!(out, "{answer}").expect("standard output");
    }
}

/// The `Query` struct, and what `run` returns followed by the items it names.
fn shown(file: syn::File) -> (String, String) {
    let mut params = None;
    let mut out = None;
    let mut named: Vec<syn::Item> = Vec::new();
    for item in file.items {
        match item {
            syn::Item::Struct(mut s) => {
                s.attrs.clear();
                for field in s.fields.iter_mut() {
                    field.attrs.clear();
                }
                if s.ident == "Params" {
                    s.ident = syn::Ident::new("Query", s.ident.span());
                    params = Some(s);
                } else {
                    named.push(syn::Item::Struct(s));
                }
            }
            syn::Item::Enum(mut e) => {
                e.attrs.clear();
                named.push(syn::Item::Enum(e));
            }
            syn::Item::Impl(i) if i.trait_.is_none() && i.self_ty.to_token_stream().to_string().starts_with("Params") => {
                for item in i.items {
                    if let syn::ImplItem::Fn(f) = item {
                        if let syn::ReturnType::Type(_, ty) = f.sig.output {
                            // `::reylo::sqlx::Result<T>`: T.
                            if let syn::Type::Path(p) = *ty {
                                if let Some(syn::PathArguments::AngleBracketed(a)) = p.path.segments.last().map(|s| s.arguments.clone()) {
                                    if let Some(syn::GenericArgument::Type(t)) = a.args.first() {
                                        out = Some(t.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let params = params.expect("a Params struct");
    let out = out.expect("a run method");
    let alias: syn::Item = syn::parse_quote!(type Returns = #out;);
    let returns = unparse(vec![alias]).trim().trim_start_matches("type Returns = ").trim_end_matches(';').to_string();
    // Each item the parameters or the row names goes under that side, in the order the attribute
    // writes them, the row first under what `run` returns.
    let mentions = |text: &str, item: &syn::Item| {
        let name = match item {
            syn::Item::Struct(s) => s.ident.to_string(),
            syn::Item::Enum(e) => e.ident.to_string(),
            _ => return false,
        };
        text.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == name)
    };
    // `Query { id: i32 }`, the fields as the struct's literal names them; a unit `Query` as it is declared.
    let params_text = if params.fields.is_empty() { unparse(vec![syn::Item::Struct(params.clone())]) } else { format!("Query {}", record(&params)) };
    let row = named.iter().position(|item| matches!(item, syn::Item::Struct(s) if s.ident == "Row")).map(|i| named.remove(i));
    let mut taken = vec![false; named.len()];
    let mut side = |seed: String| {
        let mut text = seed.clone();
        let mut parts = vec![seed];
        // Items named by items already shown, as a JSON struct inside another.
        loop {
            let next = named.iter().enumerate().find(|(i, item)| !taken[*i] && mentions(&text, item));
            let Some((i, item)) = next else { break };
            taken[i] = true;
            let shown = unparse(vec![item.clone()]).trim_end().to_string();
            text.push_str(&shown);
            parts.push(shown);
        }
        parts.join("\n\n")
    };
    let params_side = side(params_text.trim_end().to_string());
    // The row written in place, as sqlx's docs write the record `query!` returns, `{ country: String,
    // count: i64 }`: its fields with no `pub`, then the items they name.
    let returns_side = match row {
        Some(syn::Item::Struct(row)) => {
            side(replace_word(&returns, "Row", &record(&row)))
        }
        _ => returns,
    };
    (params_side, returns_side)
}

/// `text` with `word` replaced where it stands alone, not inside another name.
fn replace_word(text: &str, word: &str, with: &str) -> String {
    let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(word) {
        let before = out.chars().last().or_else(|| rest[..at].chars().last());
        let before = if at > 0 { rest[..at].chars().last() } else { before };
        let after = rest[at + word.len()..].chars().next();
        out.push_str(&rest[..at]);
        out.push_str(if ident(before) || ident(after) { word } else { with });
        rest = &rest[at + word.len()..];
    }
    out + rest
}

/// A row's fields as a record: on one line where it is short, else one field a line.
fn record(row: &syn::ItemStruct) -> String {
    let fields: Vec<String> = row
        .fields
        .iter()
        .map(|f| {
            let ty = &f.ty;
            let alias: syn::Item = syn::parse_quote!(type T = #ty;);
            let ty = unparse(vec![alias]).trim().trim_start_matches("type T = ").trim_end_matches(';').to_string();
            format!("{}: {ty}", f.ident.as_ref().map(ToString::to_string).unwrap_or_default())
        })
        .collect();
    let line = format!("{{ {} }}", fields.join(", "));
    if line.len() <= 48 && !line.contains('\n') {
        line
    } else {
        let body: Vec<String> = fields.iter().map(|f| format!("    {},", f.replace('\n', "\n    "))).collect();
        format!("{{\n{}\n}}", body.join("\n"))
    }
}

/// Items as Rust source, each path as a reader writes it.
fn unparse(items: Vec<syn::Item>) -> String {
    let text = prettyplease::unparse(&syn::File { shebang: None, attrs: Vec::new(), items });
    [
        ("::core::option::", ""),
        ("::std::vec::", ""),
        ("::std::string::", ""),
        ("::reylo::sqlx::types::chrono::", "chrono::"),
        ("::reylo::sqlx::types::Decimal", "rust_decimal::Decimal"),
        ("::reylo::sqlx::types::BigDecimal", "bigdecimal::BigDecimal"),
        ("::reylo::sqlx::types::Uuid", "uuid::Uuid"),
        ("::reylo::sqlx::types::Json", "Json"),
        ("::reylo::serde_json::", "serde_json::"),
        ("::reylo::sqlx::", "sqlx::"),
    ]
    .iter()
    .fold(text, |text, (from, to)| text.replace(from, to))
}
