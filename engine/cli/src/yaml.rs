//! The YAML `reylo analyze` prints by default: each descriptor as one document, in block style but
//! for a type, a mapping of plain values and a list of them, which are written on one line in flow
//! style where they fit (`type: { kind: integer, bits: 64, label: int8 }`, `size: { max: 1 }`). A string is plain where a YAML parser
//! reads it back as that string under YAML 1.1's rules as under 1.2's, a string of several lines a
//! literal block, and any other string double-quoted, its escapes ones JSON and YAML share. So the
//! document reads back as the descriptor's JSON, value for value.

use serde_json::{Map, Value};

/// The column a line written in flow style stays within.
const WIDTH: usize = 100;

/// The words a YAML parser reads as a boolean or a null when they stand plain, under YAML 1.1's
/// rules as under 1.2's, compared lowercased.
const RESERVED: &[&str] = &["true", "false", "yes", "no", "on", "off", "y", "n", "null"];

/// `value` as one YAML document, each line ending in a newline.
pub fn document(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Object(map) if !map.is_empty() => mapping(map, 0, &mut out),
        other => {
            out.push_str(&flow(other));
            out.push('\n');
        }
    }
    out
}

/// A mapping's entries, one per line, at `indent`.
fn mapping(map: &Map<String, Value>, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    for (key, value) in map {
        let key = scalar_text(key, false);
        out.push_str(&pad);
        out.push_str(&key);
        out.push(':');
        after(value, indent, indent + key.chars().count() + 1, out);
    }
}

/// A sequence's items, one per line, at `indent`: a mapping's first entry on its dash's line.
fn sequence(items: &[Value], indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    for item in items {
        match item {
            Value::Object(map) if !map.is_empty() && !one_line(item, indent + 1) => {
                let mut body = String::new();
                mapping(map, indent + 2, &mut body);
                out.push_str(&pad);
                out.push_str("- ");
                out.push_str(&body[indent + 2..]);
            }
            _ => {
                out.push_str(&pad);
                out.push('-');
                after(item, indent, indent + 1, out);
            }
        }
    }
}

/// A value after its key or its dash, which take `used` columns: on the same line where it is one
/// line, else on the lines below, at `indent + 2`.
fn after(value: &Value, indent: usize, used: usize, out: &mut String) {
    match value {
        Value::String(s) if literal(s) => {
            let kept = s.len() - s.trim_end_matches('\n').len();
            out.push_str(match kept {
                0 => " |-\n",
                1 => " |\n",
                _ => " |+\n",
            });
            let pad = " ".repeat(indent + 2);
            for line in s.trim_end_matches('\n').split('\n') {
                if !line.is_empty() {
                    out.push_str(&pad);
                    out.push_str(line);
                }
                out.push('\n');
            }
            // The last line's own newline is one of those it keeps; each other is an empty line.
            for _ in 1..kept.max(1) {
                out.push('\n');
            }
        }
        Value::Object(map) if !map.is_empty() && !one_line(value, used) => {
            out.push('\n');
            mapping(map, indent + 2, out);
        }
        Value::Array(items) if !items.is_empty() && !one_line(value, used) => {
            out.push('\n');
            sequence(items, indent + 2, out);
        }
        Value::Object(_) | Value::Array(_) => {
            out.push(' ');
            out.push_str(&flow(value));
            out.push('\n');
        }
        scalar => {
            out.push(' ');
            out.push_str(&scalar_value(scalar, false));
            out.push('\n');
        }
    }
}

/// Whether a mapping or a sequence after `used` columns is written on one line: an empty one, and a
/// type, a mapping of plain values or a list holding no mapping, where it fits.
fn one_line(value: &Value, used: usize) -> bool {
    let empty = match value {
        Value::Object(map) => map.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => true,
    };
    let flowing = match value {
        Value::Object(map) => map.get("kind").and_then(Value::as_str).is_some() || map.values().all(|v| !v.is_object() && !v.is_array()),
        Value::Array(items) => !items.iter().any(holds_mapping),
        _ => true,
    };
    empty || (flowing && used + 1 + flow(value).chars().count() <= WIDTH)
}

fn holds_mapping(value: &Value) -> bool {
    match value {
        Value::Object(_) => true,
        Value::Array(items) => items.iter().any(holds_mapping),
        _ => false,
    }
}

/// A value on one line, in flow style.
fn flow(value: &Value) -> String {
    match value {
        Value::Object(map) if map.is_empty() => "{}".into(),
        Value::Object(map) => format!("{{ {} }}", map.iter().map(|(k, v)| format!("{}: {}", scalar_text(k, true), flow(v))).collect::<Vec<_>>().join(", ")),
        Value::Array(items) if items.is_empty() => "[]".into(),
        Value::Array(items) => format!("[{}]", items.iter().map(flow).collect::<Vec<_>>().join(", ")),
        scalar => scalar_value(scalar, true),
    }
}

fn scalar_value(value: &Value, in_flow: bool) -> String {
    match value {
        Value::String(s) => scalar_text(s, in_flow),
        other => other.to_string(),
    }
}

/// A string as written on one line: plain where it reads back as itself, else double-quoted.
fn scalar_text(s: &str, in_flow: bool) -> String {
    if plain(s, in_flow) {
        s.to_string()
    } else {
        quoted(s)
    }
}

/// Whether a string reads back as itself written plain: it opens with a letter, `_` or `$`, so no
/// indicator, number or `.inf` opens it; it is no word a parser reads as a boolean or a null; it
/// ends with no space; and it holds no `: `, ` #` or character YAML does not print, and in flow
/// style no `:`, `,` or bracket.
fn plain(s: &str, in_flow: bool) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let Some(&first) = chars.first() else { return false };
    if !(first.is_alphabetic() || first == '_' || first == '$') || s.ends_with(' ') || RESERVED.contains(&s.to_lowercase().as_str()) {
        return false;
    }
    chars.iter().enumerate().all(|(i, &c)| match c {
        ':' => !in_flow && chars.get(i + 1).is_some_and(|next| *next != ' '),
        '#' => chars[i - 1] != ' ',
        ',' | '[' | ']' | '{' | '}' => !in_flow,
        c => printable(c),
    })
}

/// Whether YAML prints a character as itself in a plain or literal scalar: no control character
/// (a tab, DEL and the C1 range among them), none YAML 1.1 reads as a line break, nor a BOM or
/// either noncharacter YAML refuses anywhere in a stream, U+FFFE and U+FFFF.
fn printable(c: char) -> bool {
    !c.is_control() && !matches!(c, '\u{2028}' | '\u{2029}' | '\u{feff}' | '\u{fffe}' | '\u{ffff}')
}

/// Whether a string is written as a literal block: it has several lines, its first opens with no
/// space, so the block's indentation reads from it, and no line is spaces alone, which the block
/// would read as an empty line.
fn literal(s: &str) -> bool {
    let body = s.trim_end_matches('\n');
    s.contains('\n')
        && !body.is_empty()
        && !body.starts_with([' ', '\t', '\n'])
        && body.chars().all(|c| c == '\n' || c == '\t' || printable(c))
        && body.split('\n').all(|line| line.is_empty() || !line.chars().all(|c| c == ' ' || c == '\t'))
}

/// A string double-quoted, its escapes ones JSON and YAML share.
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if !printable(c) => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `value` written and read back by a YAML parser.
    fn read_back(value: &Value) -> Value {
        let text = document(value);
        serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{e}\n{text}"))
    }

    #[test]
    fn a_descriptor_reads_as_its_json_with_each_type_on_a_line() {
        let examples: Value = serde_json::from_str(include_str!("../../../spec/examples.json")).unwrap();
        for (name, example) in examples.as_object().unwrap() {
            let descriptor = &example["descriptor"];
            assert_eq!(&read_back(descriptor), descriptor, "{name}");
        }
        let first = &examples["first_example"]["descriptor"];
        let mut line = Map::new();
        line.insert("file".into(), json!("queries/get_user.sql"));
        line.extend(first.as_object().unwrap().clone());
        assert_eq!(
            document(&Value::Object(line)),
            r#"file: queries/get_user.sql
$schema: https://reylo.dev/typedefs/0.1.0.json
dialect: { name: postgres, version: "18.1" }
sql: |-
  -- One user, by id.
  select id, email, bio from users where id = $1
kind: select
comment: One user, by id.
params:
  - name: id
    type: { kind: integer, bits: 64, label: int8 }
result:
  kind: array
  size: { max: 1 }
  element:
    type:
      kind: record
      fields:
        - name: id
          type: { kind: integer, bits: 64, label: int8 }
        - name: email
          type: { kind: text, size: { max: 255 }, label: varchar }
        - name: bio
          nullable: true
          type: { kind: text, label: text }
          comment: Shown on the profile page
diagnostics: []
"#
        );
    }

    #[test]
    fn a_string_reads_back_as_itself_whatever_it_holds() {
        let strings = [
            "", "yes", "No", "NULL", "y", "true", "1", "18.1", "1e3", ".inf", "-1", "- a", "? a", "a: b", "a:b", "a #b", "a#b", "#a", "a,b", "[a]", "{a}", "a ", " a",
            "?column?", "'a'", "\"a\"", "a\\b", "~", "&a", "*a", "!a", "|a", ">a", "%a", "@a", "`a`", "a\tb", "a\u{7f}", "a\u{85}b", "a\u{2028}b", "\u{feff}a", "a\u{fffe}b", "a\u{ffff}", "a\nb\u{fffe}", "é", "日本",
            "a\nb", "a\nb\n", "a\nb\n\n", "a\n\nb", " a\nb", "\na", "a\n  b", "a\n \nb", "a\r\nb", "a\n\tb", "\n", "a\n---\nb", "a\n#b",
        ];
        for s in strings {
            let value = json!({ "s": s, "in": [s], "type": { "kind": "text", "enum": [s, "x"] } });
            assert_eq!(read_back(&value), value, "{s:?}\n{}", document(&value));
        }
    }

    #[test]
    fn a_type_too_wide_for_a_line_is_written_as_a_block() {
        let values: Vec<String> = (0..30).map(|n| format!("value{n}")).collect();
        let value = json!({ "type": { "kind": "text", "enum": values, "label": "public.mood" }, "unique": [["id"], ["email"]], "order": [{ "column": "at", "nulls": "last" }] });
        let text = document(&value);
        assert!(text.starts_with("type:\n  kind: text\n  enum:\n    - value0\n"), "{text}");
        assert!(text.contains("unique: [[id], [email]]\n") && text.contains("order:\n  - { column: at, nulls: last }\n"), "{text}");
        assert_eq!(read_back(&value), value);
    }
}
