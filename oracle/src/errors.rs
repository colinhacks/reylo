//! The errors a statement may raise, as its analysis narrows them where it runs only its
//! database's own code (`reylo_engine_core::errors`): printed into a case's `errors.txt`, where the
//! case has one, and checked against every error a run of the statement raises, in every case.

use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// The SQLSTATEs Postgres defines: the run time's list, which `.try()`'s type is written from.
fn postgres_codes() -> &'static HashSet<String> {
    static CODES: OnceLock<HashSet<String>> = OnceLock::new();
    CODES.get_or_init(|| {
        include_str!("../../src/error-codes-postgres.ts")
            .lines()
            .filter_map(|line| line.trim().strip_prefix('"')?.split('"').next().map(String::from))
            .filter(|code| code.len() == 5)
            .collect()
    })
}

/// How an error a run raised contradicts the statement's narrowed errors: a code Postgres does not
/// define, or a name one of the fields the error's code narrows does not list. Nothing where the
/// analysis narrowed nothing.
pub fn contradiction(errors: &Value, code: &str, fields: &[(&str, Option<&str>)]) -> Option<String> {
    let errors = errors.as_object()?;
    if !postgres_codes().contains(code) {
        return Some(format!("raised {code}, a code Postgres does not define, where the analysis found no code of a user's to raise it"));
    }
    let narrowed = errors.get(code)?;
    for (field, value) in fields {
        let (Some(value), Some(names)) = (value, narrowed.get(*field).and_then(Value::as_array)) else { continue };
        if !names.iter().any(|n| n.as_str() == Some(value)) {
            let listed: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
            return Some(format!("raised {code} naming {field} {value}, which its narrowed type does not list ({})", listed.join(", ")));
        }
    }
    None
}

/// The names of the errors MySQL sends a client, by number: the run time's list.
fn mysql_names() -> &'static HashMap<u16, String> {
    static NAMES: OnceLock<HashMap<u16, String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut out = HashMap::new();
        for line in include_str!("../../src/error-codes-mysql.ts").lines() {
            let Some(run) = line.trim().strip_prefix('[').and_then(|l| l.strip_suffix("]],")) else { continue };
            let Some((first, names)) = run.split_once(", [") else { continue };
            let Ok(first) = first.parse::<u16>() else { continue };
            for (k, name) in names.split(", ").enumerate() {
                out.insert(first + k as u16, name.trim_matches('"').to_string());
            }
        }
        out
    })
}

/// What a MySQL message names, as the run time reads it (`mysqlNames` in src/query-error.ts).
fn mysql_fields(errno: u16, message: &str) -> Vec<(&'static str, String)> {
    let between = |prefix: &str, suffix: &str| message.strip_prefix(prefix).and_then(|m| m.strip_suffix(suffix)).map(String::from);
    let unquote = |s: &str| s.replace("``", "`");
    let mut out = Vec::new();
    match errno {
        1062 | 1586 => {
            if let Some(at) = message.rfind("' for key '").filter(|_| message.starts_with("Duplicate entry '") && message.ends_with('\'')) {
                out.push(("key", message[at + 11..message.len() - 1].to_string()));
            }
        }
        1451 | 1452 => {
            let marker = "a foreign key constraint fails (`";
            if let Some(rest) = message.find(marker).map(|at| &message[at + marker.len()..]) {
                // `schema`.`table`, CONSTRAINT `name` ...
                if let Some((_, after)) = rest.split_once("`.`") {
                    if let Some((table, after)) = after.split_once("`, CONSTRAINT `") {
                        out.push(("table", unquote(table)));
                        if let Some((name, _)) = after.split_once('`') {
                            out.push(("constraint", unquote(name)));
                        }
                    }
                }
            }
        }
        3819 => out.extend(between("Check constraint '", "' is violated.").map(|c| ("constraint", c))),
        1048 => out.extend(between("Column '", "' cannot be null").map(|c| ("column", c))),
        1364 => out.extend(between("Field '", "' doesn't have a default value").map(|c| ("column", c))),
        1264 | 1265 | 1292 | 1366 | 1406 => {
            if let (Some(at), Some(row)) = (message.rfind(" for column '"), message.rfind("' at row ")) {
                if row > at && message[row + 9..].bytes().all(|b| b.is_ascii_digit()) {
                    out.push(("column", message[at + 13..row].to_string()));
                }
            }
        }
        _ => {}
    }
    out
}

/// How a MySQL error a run raised contradicts the statement's narrowed errors: a name one of the
/// fields its code narrows does not list. MySQL's code may be one its list lacks, a closed
/// statement's too (a server error of the 1 to 999 range), so the code itself is no contradiction.
pub fn mysql_contradiction(errors: &Value, errno: u16, message: &str) -> Option<String> {
    let errors = errors.as_object()?;
    let code = mysql_names().get(&errno)?;
    let narrowed = errors.get(code)?;
    for (field, value) in mysql_fields(errno, message) {
        let Some(names) = narrowed.get(field).and_then(Value::as_array) else { continue };
        if !names.iter().any(|n| n.as_str() == Some(value.as_str())) {
            let listed: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
            return Some(format!("raised {code} naming {field} {value}, which its narrowed type does not list ({})", listed.join(", ")));
        }
    }
    None
}

/// The names of SQLite's result codes, by number: the run time's list.
fn sqlite_names() -> &'static HashMap<i32, String> {
    static NAMES: OnceLock<HashMap<i32, String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        include_str!("../../src/error-codes-sqlite.ts")
            .lines()
            .filter_map(|line| {
                let (name, code) = line.trim().strip_suffix(',')?.split_once(": ")?;
                Some((code.parse::<i32>().ok()?, name.to_string()))
            })
            .filter(|(_, name)| name.starts_with("SQLITE_"))
            .collect()
    })
}

/// What a SQLite message names for a constraint it failed, as the run time reads it (`sqliteNames`
/// in src/query-error.ts): the columns as `a, b`.
fn sqlite_fields(message: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(index) = message.strip_prefix("UNIQUE constraint failed: index '").and_then(|m| m.strip_suffix('\'')) {
        out.push(("constraint", index.to_string()));
    } else if let Some(columns) = message.strip_prefix("UNIQUE constraint failed: ").or_else(|| message.strip_prefix("NOT NULL constraint failed: ")) {
        let pairs: Vec<Option<(&str, &str)>> = columns.split(", ").map(|c| c.split_once('.')).collect();
        if let Some(Some((table, _))) = pairs.first() {
            if pairs.iter().all(|p| p.is_some_and(|(t, _)| t == *table)) {
                out.push(("table", table.to_string()));
                out.push(("columns", pairs.iter().map(|p| p.map_or("", |(_, c)| c)).collect::<Vec<_>>().join(", ")));
            }
        }
    } else if let Some(rest) = message.strip_prefix("cannot store ") {
        if let Some((table, column)) = rest.rsplit_once(" column ").and_then(|(_, tc)| tc.split_once('.')) {
            out.push(("table", table.to_string()));
            out.push(("columns", column.to_string()));
        }
    }
    out
}

/// How a SQLite error a run raised contradicts the statement's narrowed errors: a name one of the
/// fields its extended code narrows does not list.
pub fn sqlite_contradiction(errors: &Value, extended: i32, message: &str) -> Option<String> {
    let errors = errors.as_object()?;
    if extended & 0xff != 19 {
        return None;
    }
    let code = sqlite_names().get(&extended)?;
    let narrowed = errors.get(code)?;
    for (field, value) in sqlite_fields(message) {
        let Some(names) = narrowed.get(field).and_then(Value::as_array) else { continue };
        if !names.iter().any(|n| n.as_str() == Some(value.as_str())) {
            let listed: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
            return Some(format!("raised {code} naming {field} {value}, which its narrowed type does not list ({})", listed.join("; ")));
        }
    }
    None
}

/// One line per statement of a case: `open` where its errors are not narrowed, else `closed` and
/// each narrowed code with the names of each of its fields, as a JSON list (a SQLite unique key's
/// columns are one name, `a, b`).
pub fn lines(case: &Path, files: &[String], analyses: &Map<String, Value>) -> Vec<String> {
    let name = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    files
        .iter()
        .map(|file| {
            let errors = analyses.get(&format!("{name}/{file}")).and_then(|a| a.get("errors")).and_then(Value::as_object);
            let Some(errors) = errors else { return format!("{file}: open") };
            let mut out = format!("{file}: closed");
            for (code, fields) in errors {
                let fields: Vec<String> = fields
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(field, names)| format!("{field} {names}"))
                    .collect();
                out.push_str(&format!(" | {code} {}", fields.join("; ")));
            }
            out
        })
        .collect()
}

/// Compare a case's lines with its `errors.txt`, or write them there under `--bless`; a case
/// without the file is not compared.
pub fn check(case: &Path, lines: &[String], bless: bool) -> bool {
    let path = case.join("errors.txt");
    if !path.is_file() {
        return true;
    }
    let actual = lines.join("\n") + "\n";
    if bless {
        fs::write(&path, &actual).expect("errors.txt is writable");
        return true;
    }
    let expected = fs::read_to_string(&path).unwrap_or_default();
    if expected == actual {
        return true;
    }
    println!("{}: MISMATCH (errors.txt)", case.display());
    let want: Vec<&str> = expected.lines().collect();
    for line in lines {
        if !want.contains(&line.as_str()) {
            println!("  + {line}");
        }
    }
    for line in &want {
        if !lines.iter().any(|l| l == line) {
            println!("  - {line}");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_error_outside_the_narrowing_contradicts_it() {
        let errors = json!({ "23505": { "constraint": ["users_email_key", "users_pkey"], "table": ["users"] } });
        assert_eq!(contradiction(&errors, "23505", &[("constraint", Some("users_pkey")), ("table", Some("users"))]), None);
        assert_eq!(contradiction(&errors, "23505", &[("constraint", None)]), None);
        assert_eq!(contradiction(&errors, "22012", &[("constraint", Some("anything"))]), None);
        assert!(contradiction(&errors, "23505", &[("constraint", Some("posts_pkey"))]).is_some());
        assert!(contradiction(&errors, "U0001", &[]).is_some());
        assert_eq!(contradiction(&Value::Null, "U0001", &[]), None);
    }

    #[test]
    fn a_mysql_message_names_what_the_run_time_reads() {
        assert_eq!(mysql_names().get(&1062).map(String::as_str), Some("ER_DUP_ENTRY"));
        assert_eq!(mysql_fields(1062, "Duplicate entry 'a' for key 't.email'"), vec![("key", "t.email".to_string())]);
        assert_eq!(mysql_fields(1452, "Cannot add or update a child row: a foreign key constraint fails (`db`.`t`, CONSTRAINT `t_pid` FOREIGN KEY (`pid`) REFERENCES `parent` (`id`))"), vec![("table", "t".to_string()), ("constraint", "t_pid".to_string())]);
        assert_eq!(mysql_fields(3819, "Check constraint 'n_positive' is violated."), vec![("constraint", "n_positive".to_string())]);
        assert_eq!(mysql_fields(1406, "Data too long for column 'email' at row 1"), vec![("column", "email".to_string())]);
        let errors = serde_json::json!({ "ER_DUP_ENTRY": { "key": ["t.PRIMARY", "t.email"] } });
        assert_eq!(mysql_contradiction(&errors, 1062, "Duplicate entry 'a' for key 't.email'"), None);
        assert!(mysql_contradiction(&errors, 1062, "Duplicate entry 'a' for key 'u.email'").is_some());
    }

    #[test]
    fn a_sqlite_message_names_what_the_run_time_reads() {
        assert_eq!(sqlite_names().get(&2067).map(String::as_str), Some("SQLITE_CONSTRAINT_UNIQUE"));
        assert_eq!(sqlite_fields("UNIQUE constraint failed: t.a, t.b"), vec![("table", "t".to_string()), ("columns", "a, b".to_string())]);
        assert_eq!(sqlite_fields("UNIQUE constraint failed: index 'users_lower'"), vec![("constraint", "users_lower".to_string())]);
        assert_eq!(sqlite_fields("cannot store TEXT value in INTEGER column s.x"), vec![("table", "s".to_string()), ("columns", "x".to_string())]);
        let errors = serde_json::json!({ "SQLITE_CONSTRAINT_UNIQUE": { "columns": ["a, b", "email"], "table": ["t"] } });
        assert_eq!(sqlite_contradiction(&errors, 2067, "UNIQUE constraint failed: t.a, t.b"), None);
        assert!(sqlite_contradiction(&errors, 2067, "UNIQUE constraint failed: t.c").is_some());
    }
}
