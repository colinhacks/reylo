//! The format's own rules, held on every type the engine gives: each parameter's and column's type is a `Type` of the format's JSON Schema, spec/typedefs.schema.json, reads as the format's [`Type`] and writes again as the same JSON, and passes the validity rules, rule 7 of spec/README.md ([`Type::check`]). Nested JSON shapes, unions and structured parameters are built for the statement, so no table of a database's built-in types covers them. A case with a type that breaks a rule fails, whatever else it shows; a replay counts each as a contradiction.

use reylo_engine_core::types::Type;
use serde_json::{Map, Value};
use std::path::Path;
use std::sync::OnceLock;

/// The format's JSON Schema, compiled once, and the index of its definition of a type. A duration's `max` takes a `pattern` with a lookahead, which this validator's regular expressions cannot read, so the pattern is left out here: [`Type::check`] holds `max` to that form and to its canonical spelling.
fn schema() -> &'static (boon::Schemas, boon::SchemaIndex) {
    static SCHEMA: OnceLock<(boon::Schemas, boon::SchemaIndex)> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let mut source: Value = serde_json::from_str(include_str!("../../spec/typedefs.schema.json")).expect("the format's JSON Schema is JSON");
        if let Some(Value::Object(max)) = source.pointer_mut("/$defs/DurationType/properties/max") {
            max.remove("pattern");
        }
        let id = source["$id"].as_str().expect("the format's JSON Schema names its $id").to_string();
        let mut schemas = boon::Schemas::new();
        let mut compiler = boon::Compiler::new();
        compiler.add_resource(&id, source).expect("the format's JSON Schema is added once");
        let index = compiler.compile(&format!("{id}#/$defs/Type"), &mut schemas).unwrap_or_else(|e| panic!("the format's JSON Schema compiles: {e:#}"));
        (schemas, index)
    })
}

/// Each way a parameter's or a column's type in `analysis` breaks the format's rules, as `column <name>: <why>`.
pub fn violations(analysis: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (list, what) in [("params", "parameter"), ("columns", "column")] {
        for position in analysis[list].as_array().into_iter().flatten() {
            let at = format!("{what} {}", position["name"].as_str().unwrap_or("?"));
            let written = &position["type"];
            let (schemas, index) = schema();
            if let Err(e) = schemas.validate(written, *index) {
                out.push(format!("{at}: its type is no Type of the format's JSON Schema: {}", cause(&e)));
            }
            let ty = match serde_json::from_value::<Type>(written.clone()) {
                Ok(ty) => ty,
                Err(e) => {
                    out.push(format!("{at}: its type does not read as the format's: {e}: {}", cut(written)));
                    continue;
                }
            };
            match serde_json::to_value(&ty) {
                Ok(again) if again == *written => {}
                Ok(again) => out.push(format!("{at}: its type {} reads back as {}", cut(written), cut(&again))),
                Err(e) => out.push(format!("{at}: its type does not write again: {e}")),
            }
            if let Err(e) = ty.check() {
                out.push(format!("{at}: {e}"));
            }
        }
    }
    out
}

/// Holds the analysis of each of a case's statements to the format's rules, printing every violation; whether there was none.
pub fn check(case: &Path, files: &[String], analyses: &Map<String, Value>) -> bool {
    let name = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let found: Vec<String> = files
        .iter()
        .filter_map(|file| Some((file, analyses.get(&format!("{name}/{file}"))?)))
        .flat_map(|(file, analysis)| violations(analysis).into_iter().map(move |v| format!("{file}: {v}")))
        .collect();
    if !found.is_empty() {
        println!("{}: INVALID: {} type(s) break the format's rules:", case.display(), found.len());
        for line in &found {
            println!("    {line}");
        }
    }
    found.is_empty()
}

/// What a refusal of the schema says where it is most precise. A type is a union of every kind (`anyOf`), which each kind but the value's own refuses at its `kind`: those are passed over, and the refusal followed down is the value's own kind's.
fn cause(e: &boon::ValidationError) -> String {
    /// Whether `e`, a kind of a union at `at`, refuses the value's `kind`.
    fn other_kind(e: &boon::ValidationError, at: &str) -> bool {
        (matches!(e.kind, boon::ErrorKind::Const { .. }) && e.instance_location.to_string() == format!("{at}/kind")) || e.causes.iter().any(|c| other_kind(c, at))
    }
    let mut e = e;
    loop {
        let at = e.instance_location.to_string();
        let next = match e.kind {
            boon::ErrorKind::AnyOf => match e.causes.iter().find(|c| !other_kind(c, &at)) {
                Some(own) => Some(own),
                None => return format!("at {at:?}: its kind is none this position takes"),
            },
            _ => e.causes.first(),
        };
        match next {
            Some(next) => e = next,
            // A key no definition of its kind evaluates meets the `false` of `unevaluatedProperties`.
            None if matches!(e.kind, boon::ErrorKind::FalseSchema) => return format!("at {at:?}: a field the format does not define"),
            None => return format!("at {at:?}: {}", e.kind),
        }
    }
}

/// A type's JSON on one line, cut to a length a report can show.
fn cut(v: &Value) -> String {
    let text = v.to_string();
    match text.char_indices().nth(300) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_type_that_breaks_a_rule_or_does_not_read_back_is_named_with_its_position() {
        let valid = json!({"params": [{"name": "id", "type": {"kind": "integer", "bits": 64}}], "columns": [{"name": "n", "nullable": true, "type": {"kind": "json", "content": {"type": {"kind": "array", "element": {"type": {"kind": "text"}}}}}}]});
        assert_eq!(violations(&valid), Vec::<String>::new());
        let invalid = json!({"params": [], "columns": [
            {"name": "a", "type": {"kind": "json", "content": {"type": {"kind": "text", "size": {"min": 4, "max": 4}}}}},
            {"name": "b", "type": {"kind": "integer", "bits": 32, "unsigned": false}},
            {"name": "c", "type": {"kind": "no such kind"}},
        ]});
        let found: Vec<String> = violations(&invalid).into_iter().filter(|v| !v.contains("JSON Schema")).collect();
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(found[0], "column a: <json> (text): a size's min 4 is not below its max 4");
        // A false flag is written by leaving it out.
        assert!(found[1].starts_with("column b: its type ") && found[1].contains("reads back as"), "{}", found[1]);
        assert!(found[2].starts_with("column c: its type does not read as the format's"), "{}", found[2]);
        // A type the schema refuses, as a field it does not define, though serde reads past it.
        let unknown = json!({"columns": [{"name": "d", "type": {"kind": "array", "element": {"type": {"kind": "text", "colour": "red"}}}}]});
        let found = violations(&unknown);
        assert!(found.iter().any(|v| v == "column d: its type is no Type of the format's JSON Schema: at \"/element/type/colour\": a field the format does not define"), "{found:?}");
        // A kind no position takes, which every kind of the union refuses at its `kind`.
        let no_kind = json!({"columns": [{"name": "e", "type": {"kind": "array", "element": {"type": {"kind": "no such kind"}}}}]});
        let found = violations(&no_kind);
        assert!(found.iter().any(|v| v == "column e: its type is no Type of the format's JSON Schema: at \"/element/type\": its kind is none this position takes"), "{found:?}");
    }
}
