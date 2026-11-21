//! The ClickHouse half: the host the engine's replay asks, and the runs.
//!
//! The host is the native program's own (`reylo_engine::clients::clickhouse`, the server's HTTP
//! interface), and the engine's needs are queries it sends as they are written. A case runs in two
//! databases of its own, made on the server `--clickhouse-url` (or `ORACLE_CLICKHOUSE_URL`) names,
//! which must be a throwaway one: it is refused on ports 8123 and 8443, ClickHouse's own. One is
//! what `setup.sql` makes, its statements parted by a `;` at a line's end; the other is the same
//! with no `INSERT` run, so every table is empty there, which is where a row count the engine
//! claims is most easily wrong.
//!
//! ClickHouse has no transaction to roll a run back in, so a statement that reads runs as often as
//! its parameters have values, on both databases, and a statement that writes runs once, on the
//! first, after every read. A parameter's values are its type's own: an integer's ends, a float's
//! NaN and infinity, a decimal's largest, the empty string and one past ASCII, the first and last
//! date and instant, each label of an enum, NULL where the type takes it, an array of none and of
//! one. Each goes as the text the run time writes for it (`param_<name>`).
//!
//! Every run that succeeded is checked. The types the result came back with are the ones the
//! analysis read, by name, which is the claim ClickHouse makes hardest: a parameter's value, or a
//! table's contents, changing a column's type would show here. The rows are within the
//! cardinality. No value is NULL outside a `Nullable`, in a column or inside an array, a tuple or
//! a map, an enum's value is one of its labels, and each value is within its type descriptor (a
//! decimal's digits, an instant's tick).

use crate::seed::{assignments, contradiction, report, Report, Tally};
use reylo_engine::clients::clickhouse::{self as answers, Outcome, Server as Http};
use reylo_engine::engines::clickhouse::types::{self, Arg, Named};
use reylo_engine::AnswerCache;
use reylo_engine_core::dialect::Dialect;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

pub struct Server {
    admin: Http,
    url: String,
    /// The version as one number: 260923 for 26.9.23.
    pub version: i32,
}

/// `url` with its database named `db`.
fn with_database(url: &str, db: &str) -> String {
    let Some(scheme) = url.find("://") else { return url.to_string() };
    let rest = &url[scheme + 3..];
    let authority = rest.find(['/', '?']).unwrap_or(rest.len());
    let query = rest[authority..].find('?').map(|q| &rest[authority + q..]).unwrap_or("");
    format!("{}{}/{db}{query}", &url[..scheme + 3], &rest[..authority])
}

fn port_of(url: &str) -> Option<u16> {
    let rest = &url[url.find("://")? + 3..];
    let authority = &rest[..rest.find(['/', '?']).unwrap_or(rest.len())];
    authority.rsplit('@').next()?.rsplit_once(':').and_then(|(_, port)| port.parse().ok())
}

/// A statement's rows and their columns, or the server's error.
fn ask(http: &Http, sql: &str, fields: &[(String, String)]) -> Result<(Vec<(String, String)>, Vec<Vec<Value>>), String> {
    match http.ask_typed(sql, fields)? {
        Outcome::Typed(columns, rows) => Ok((columns, rows)),
        Outcome::Rows(rows) => Ok((Vec::new(), rows)),
        Outcome::Refused { message, .. } => Err(message),
        Outcome::Foreign(what) => Err(format!("the server answered with {what}")),
    }
}

fn exec(http: &Http, sql: &str) -> Result<(), String> {
    ask(http, sql, &[]).map(|_| ())
}

pub fn connect(url: &str) -> Result<Server, String> {
    if matches!(port_of(url), Some(8123 | 8443)) {
        return Err("ports 8123 and 8443 are ClickHouse's own; the oracle runs only on a throwaway server on another port".into());
    }
    let admin = Http::of(&with_database(url, "default"))?;
    let (_, rows) = ask(&admin, "SELECT version()", &[])?;
    let text = rows.first().and_then(|row| row.first()).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut parts = text.split('.').map(|part| part.parse::<i32>().unwrap_or(0));
    let version = parts.next().unwrap_or(0) * 10_000 + parts.next().unwrap_or(0) * 100 + parts.next().unwrap_or(0);
    Ok(Server { admin, url: url.to_string(), version })
}

/// A script's statements, parted by a `;` at a line's end.
fn script(sql: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in sql.lines() {
        let last = out.last_mut().expect("one statement at least");
        match line.trim_end().strip_suffix(';') {
            Some(before) => {
                last.push_str(before);
                out.push(String::new());
            }
            None => {
                last.push_str(line);
                last.push('\n');
            }
        }
    }
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

fn first_word(sql: &str) -> String {
    sql.trim_start().chars().take_while(char::is_ascii_alphabetic).collect::<String>().to_ascii_lowercase()
}

/// A case's two databases, made anew: the one its `setup.sql` seeds, then the one that holds its
/// tables and no row. Their names, and a client of each.
fn build(case: &Path, server: &Server) -> Result<([String; 2], Vec<Http>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let db = format!("reylo_oracle_{}", name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>());
    let empty = format!("{db}_empty");
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    let mut states = Vec::new();
    for (database, seeded) in [(&db, true), (&empty, false)] {
        exec(&server.admin, &format!("DROP DATABASE IF EXISTS {database}")).map_err(|e| format!("drop {database}: {e}"))?;
        exec(&server.admin, &format!("CREATE DATABASE {database}")).map_err(|e| format!("create {database}: {e}"))?;
        let http = Http::of(&with_database(&server.url, database))?;
        for statement in script(&setup) {
            if seeded || first_word(&statement) != "insert" {
                exec(&http, &statement).map_err(|e| format!("setup.sql: {e}"))?;
            }
        }
        states.push(http);
    }
    Ok(([db, empty], states))
}

/// A ClickHouse case's mutations, checked over its two databases as the case's statements are.
pub fn mutate(case: &Path, server: &mut Server, timeout_ms: u64, mutations: &[(usize, &crate::Mutation)]) -> Result<Report, String> {
    let (databases, states) = build(case, server)?;
    let outcome: Result<Report, String> = (|| {
        let mut analyzed = Vec::new();
        for (n, m) in mutations {
            let sql = fs::read_to_string(case.join(&m.file)).map_err(|e| format!("{}: {e}", m.file))?;
            let analysis = answers::analyze(&states[0], &sql, &mut AnswerCache::default()).map_err(|e| format!("{}: {e}", m.file))?;
            analyzed.push((m.label(*n), sql, m.apply(serde_json::to_value(&analysis).map_err(|e| e.to_string())?)?));
        }
        Ok(check(&states, &analyzed, timeout_ms))
    })();
    for database in &databases {
        let _ = exec(&server.admin, &format!("DROP DATABASE IF EXISTS {database}"));
    }
    outcome
}

pub fn run(case: &Path, server: &mut Server, keep: bool, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>, statements: Vec<(String, String)>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (databases, states) = build(case, server)?;
    let mut lines = Vec::new();
    let mut analyzed = Vec::new();
    for (file, sql) in statements {
        let analysis = match given.get(&format!("{name}/{file}")) {
            Some(answer) => answer.clone(),
            None => {
                let analysis = answers::analyze(&states[0], &sql, &mut AnswerCache::default()).map_err(|e| format!("{file}: {e}"))?;
                serde_json::to_value(&analysis).map_err(|e| e.to_string())?
            }
        };
        lines.push(format!("{file}: {}", crate::line(&lasting(&analysis))));
        dumped.insert(format!("{name}/{file}"), analysis.clone());
        analyzed.push((file, sql, analysis));
    }
    // A value that contradicts a type is a failure whatever expected.txt says.
    let report = check(&states, &analyzed, timeout_ms);
    if !keep {
        for database in &databases {
            let _ = exec(&server.admin, &format!("DROP DATABASE IF EXISTS {database}"));
        }
    }
    let unchecked = crate::seeded(case, lines.len(), report)?;
    Ok((lines, unchecked))
}

/// The name the server gives an error at the end of its message, `(UNKNOWN_IDENTIFIER)`, and
/// what follows it.
fn error_name(message: &str) -> Option<&str> {
    message.rmatch_indices('(').map(|(at, _)| &message[at..]).find(|rest| {
        let name = rest[1..].split(')').next().unwrap_or_default();
        name.len() > 2 && rest[1 + name.len()..].starts_with(')') && name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// An analysis as a case's expected.txt holds it, on every version of the server: a diagnostic
/// that is the server's error is its name alone, since the server's wording of one changes from a
/// release to the next, and one that quotes the server's error after its own words ends before the quote.
fn lasting(analysis: &Value) -> Value {
    let mut out = analysis.clone();
    for d in out["diagnostics"].as_array_mut().into_iter().flatten() {
        let message = d["message"].as_str().unwrap_or_default().to_string();
        let kept = if d["server_code"].is_string() {
            error_name(&message).map(str::to_string)
        } else {
            message.split_once(" in the statement. (").map(|(before, _)| format!("{before} in the statement."))
        };
        if let Some(kept) = kept {
            d["message"] = Value::String(kept);
        }
    }
    out
}

/// A run's error as a case's unchecked.txt holds it: without the version it ends with.
fn unversioned(error: &str) -> String {
    error.rfind(" (version ").map_or(error, |at| &error[..at]).to_string()
}

// ---------------------------------------------------------------------------------------------
// A parameter's values
// ---------------------------------------------------------------------------------------------

/// The ends of an integer of `bits`, as text.
fn integer_ends(bits: u32, unsigned: bool) -> (String, String) {
    let pow = |n: u32| {
        // 2^n in decimal, by doubling its digits.
        let mut digits = vec![1u8];
        for _ in 0..n {
            let mut carry = 0;
            for d in digits.iter_mut() {
                let v = *d * 2 + carry;
                *d = v % 10;
                carry = v / 10;
            }
            if carry > 0 {
                digits.push(carry);
            }
        }
        digits
    };
    let text = |digits: &[u8]| digits.iter().rev().map(|d| char::from(b'0' + d)).collect::<String>();
    // 2^n − 1: a power of two never ends in 0.
    let less_one = |mut digits: Vec<u8>| {
        digits[0] -= 1;
        text(&digits)
    };
    if unsigned {
        ("0".into(), less_one(pow(bits)))
    } else {
        (format!("-{}", text(&pow(bits - 1))), less_one(pow(bits - 1)))
    }
}

fn integer(name: &str) -> Option<(u32, bool)> {
    let (digits, unsigned) = match name.strip_prefix("UInt") {
        Some(digits) => (digits, true),
        None => (name.strip_prefix("Int")?, false),
    };
    digits.parse().ok().filter(|bits| matches!(bits, 8 | 16 | 32 | 64 | 128 | 256)).map(|bits| (bits, unsigned))
}

fn number(t: &Named, at: usize) -> Option<usize> {
    match t.args.get(at)? {
        Arg::Number(n) => n.parse().ok(),
        _ => None,
    }
}

/// A string as the text of a parameter's whole value: a backslash before what one writes.
fn escaped(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

/// The values a parameter of the type takes in the runs, each as the text of its whole value, or
/// `None` for a type the oracle has no values of.
fn pool(t: &Named) -> Option<Vec<String>> {
    let texts = |values: &[&str]| Some(values.iter().map(|v| v.to_string()).collect::<Vec<_>>());
    if let Some((bits, unsigned)) = integer(&t.name) {
        let (min, max) = integer_ends(bits, unsigned);
        return Some(if unsigned { vec![min, "1".into(), max] } else { vec![min, "-1".into(), "0".into(), "1".into(), max] });
    }
    match (t.name.as_str(), t.args.as_slice()) {
        ("Nullable", [Arg::Type(inner)]) => {
            let mut values = pool(inner)?;
            values.push("\\N".into());
            Some(values)
        }
        ("LowCardinality", [Arg::Type(inner)]) => pool(inner),
        ("Bool", _) => texts(&["true", "false"]),
        ("Float32" | "Float64", _) => texts(&["0", "-1.5", "1e30", "nan", "inf", "-inf"]),
        ("Decimal" | "Decimal32" | "Decimal64" | "Decimal128" | "Decimal256", _) => {
            let (precision, scale) = match t.name.as_str() {
                "Decimal" => (number(t, 0)?, number(t, 1).unwrap_or(0)),
                "Decimal32" => (9, number(t, 0)?),
                "Decimal64" => (18, number(t, 0)?),
                "Decimal128" => (38, number(t, 0)?),
                _ => (76, number(t, 0)?),
            };
            let whole = "9".repeat(precision.saturating_sub(scale));
            let largest = match (whole.is_empty(), scale) {
                (true, _) => format!("0.{}", "9".repeat(scale)),
                (false, 0) => whole,
                (false, _) => format!("{whole}.{}", "9".repeat(scale)),
            };
            // One, or the largest value where the type holds none as large.
            let one = if precision > scale { "1".to_string() } else { largest.clone() };
            Some(vec!["0".into(), one, format!("-{largest}"), largest])
        }
        // Texts of every kind, and the ones a statement computes with: a date, a time, a zone, and a unit of a time and one of a date.
        ("String", _) => Some(["", "a", "A ", "é😀", "it's \\ a\tb", "2026-01-02", "2026-01-02 03:04:05", "UTC", "day", "month"].iter().map(|s| escaped(s)).collect()),
        ("FixedString", _) => texts(&["", "a"]),
        ("UUID", _) => texts(&["00000000-0000-0000-0000-000000000000", "61f0c404-5cb3-11e7-907b-a6006ad3dba0"]),
        ("IPv4", _) => texts(&["0.0.0.0", "255.255.255.255"]),
        ("IPv6", _) => texts(&["::", "2001:db8::1"]),
        ("Date", _) => texts(&["1970-01-01", "2026-01-02", "2149-06-06"]),
        ("Date32", _) => texts(&["1900-01-01", "2026-01-02", "2299-12-31"]),
        // Its seconds since 1970, in five digits or more: the server reads a shorter number as a year.
        ("DateTime", _) => texts(&["00000", "1767268800", "4294967295"]),
        ("DateTime64", _) => {
            let digits = number(t, 0)?;
            let fraction = if digits == 0 { String::new() } else { format!(".{}", "1".repeat(digits)) };
            Some(vec![format!("00000{}", fraction.replace('1', "0")), format!("1767268800{fraction}"), format!("-00001{fraction}")])
        }
        ("Enum8" | "Enum16", labels) => labels.iter().map(|a| if let Arg::Label(label, _) = a { Some(escaped(label)) } else { None }).collect(),
        ("Array", [Arg::Type(inner)]) => Some(vec!["[]".into(), format!("[{}]", inside(inner)?)]),
        ("Tuple", _) | ("Map", _) => Some(vec![inside(t)?]),
        _ => None,
    }
}

/// One value of the type as it is written inside an array, a tuple or a map: a string in quotes.
fn inside(t: &Named) -> Option<String> {
    let quoted = |text: &str| format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"));
    if integer(&t.name).is_some() {
        return Some("1".into());
    }
    match (t.name.as_str(), t.args.as_slice()) {
        ("Nullable", _) => Some("NULL".into()),
        ("LowCardinality", [Arg::Type(inner)]) => inside(inner),
        ("Bool", _) => Some("true".into()),
        ("Float32" | "Float64", _) => Some("-1.5".into()),
        ("Decimal" | "Decimal32" | "Decimal64" | "Decimal128" | "Decimal256", _) => Some("1".into()),
        ("String", _) => Some(quoted("it's")),
        ("FixedString", _) => Some(quoted("a")),
        ("UUID", _) => Some(quoted("61f0c404-5cb3-11e7-907b-a6006ad3dba0")),
        ("IPv4", _) => Some(quoted("1.2.3.4")),
        ("IPv6", _) => Some(quoted("2001:db8::1")),
        ("Date" | "Date32", _) => Some(quoted("2026-01-02")),
        ("DateTime", _) => Some("1767268800".into()),
        ("DateTime64", _) => Some(quoted("1767268800")),
        ("Enum8" | "Enum16", [Arg::Label(label, _), ..]) => Some(quoted(label)),
        ("Array", [Arg::Type(inner)]) => Some(format!("[{}]", inside(inner)?)),
        ("Tuple", members) => {
            let values: Option<Vec<String>> = members.iter().map(|a| match a {
                Arg::Type(m) | Arg::Member(_, m) => inside(m),
                _ => None,
            }).collect();
            Some(format!("({})", values?.join(",")))
        }
        ("Map", [Arg::Type(key), Arg::Type(value)]) => Some(format!("{{{}:{}}}", inside(key)?, inside(value)?)),
        _ => None,
    }
}

/// Each parameter of the statement as it is sent, by name, with the type its placeholder declares.
fn parameters(sent: &str) -> Result<Vec<(String, Named)>, String> {
    let lexed = reylo_engine_core::lex::lex(sent, Dialect::Clickhouse).map_err(|e| e.message)?;
    let mut out: Vec<(String, Named)> = Vec::new();
    for p in &lexed.placeholders {
        let written = &sent[p.start..p.end];
        let declared = written.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')).and_then(|inner| inner.split_once(':')).map(|(_, ty)| ty.trim());
        let ty = declared.and_then(types::parse).ok_or_else(|| format!("{written} declares no type"))?;
        if !out.iter().any(|(name, _)| name == &p.name) {
            out.push((p.name.clone(), ty));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// The runs
// ---------------------------------------------------------------------------------------------

/// The settings every run carries: a number past 2^53, a decimal and a float that is no number as
/// their exact text, a map as its pairs, and the time the run may take.
fn settings(timeout_ms: u64) -> Vec<(String, String)> {
    [
        ("output_format_json_quote_64bit_integers", "1".to_string()),
        ("output_format_json_quote_decimals", "1".to_string()),
        ("output_format_json_quote_denormals", "1".to_string()),
        ("output_format_json_map_as_array_of_tuples", "1".to_string()),
        ("max_execution_time", timeout_ms.div_ceil(1000).max(1).to_string()),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
    .collect()
}

/// The type a value's JSON is read by: without the wrappers that change no value.
fn plain(t: &Named) -> &Named {
    match (t.name.as_str(), t.args.as_slice()) {
        ("LowCardinality", [Arg::Type(inner)]) => plain(inner),
        ("SimpleAggregateFunction", [_, Arg::Type(inner)]) => plain(inner),
        _ => t,
    }
}

/// Why a value contradicts the type it came back under, if it does: a NULL outside a `Nullable`,
/// at any depth, or a label an enum does not have.
fn value_problem(t: &Named, value: &Value, at: &str) -> Option<String> {
    let t = plain(t);
    match (t.name.as_str(), t.args.as_slice()) {
        ("Nullable", [Arg::Type(inner)]) => (!value.is_null()).then(|| value_problem(inner, value, at)).flatten(),
        // A Variant holds NULL itself, and nothing is said of the value of a type no describe names.
        ("Variant" | "Nothing" | "Dynamic" | "JSON", _) => None,
        _ if value.is_null() => Some(format!("holds NULL{at}, in a {t} that is no Nullable")),
        ("Array", [Arg::Type(inner)]) => value.as_array()?.iter().enumerate().find_map(|(i, v)| value_problem(inner, v, &format!("{at} at [{i}]"))),
        ("Tuple", members) => members.iter().enumerate().find_map(|(i, a)| match a {
            // A tuple with names is an object of them, and one without an array.
            Arg::Member(name, m) => value_problem(m, value.get(name.as_str()).or_else(|| value.get(i))?, &format!("{at} at .{name}")),
            Arg::Type(m) => value_problem(m, value.get(i)?, &format!("{at} at [{i}]")),
            _ => None,
        }),
        ("Map", [Arg::Type(key), Arg::Type(inner)]) => value.as_array()?.iter().enumerate().find_map(|(i, pair)| value_problem(key, pair.get(0)?, &format!("{at} at [{i}]'s key")).or_else(|| value_problem(inner, pair.get(1)?, &format!("{at} at [{i}]'s value")))),
        ("Enum8" | "Enum16", labels) => {
            let label = value.as_str()?;
            (!labels.iter().any(|a| matches!(a, Arg::Label(l, _) if l == label))).then(|| format!("holds {label:?}{at}, which {t} has no label for"))
        }
        _ => None,
    }
}

/// A scalar's JSON as the text the descriptor's own checks read.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn check(states: &[Http], analyzed: &[(String, String, Value)], timeout_ms: u64) -> Report {
    let mut problems = Vec::new();
    let mut tallies: Vec<Tally> = analyzed.iter().map(|_| Tally::default()).collect();
    let fields = settings(timeout_ms);
    // The reads first, on both databases; then each write once, on the first.
    for writes in [false, true] {
        for ((file, _, analysis), tally) in analyzed.iter().zip(tallies.iter_mut()) {
            if analysis["diagnostics"].as_array().is_some_and(|d| !d.is_empty()) {
                tally.refused = Some("the engine refuses it".into());
                continue;
            }
            let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
            let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
            if (cardinality == "none") != writes {
                continue;
            }
            let sent = analysis["rewritten"]["sql"].as_str().unwrap_or_default();
            let params = match parameters(sent) {
                Ok(params) => params,
                Err(why) => {
                    tally.refused = Some(why);
                    continue;
                }
            };
            let pools: Option<Vec<Vec<String>>> = params.iter().map(|(_, ty)| pool(ty)).collect();
            let Some(pools) = pools else {
                tally.refused = Some("a parameter's type has no values the oracle writes".into());
                continue;
            };
            // A pool always holds a value, so the stand-in for an empty one is never bound.
            let all = assignments(&pools, &"\\N".to_string());
            // Each run with its state's place: the seeded database is the first, the empty one the second.
            let runs: Vec<(usize, &Http, &Vec<String>)> = if writes { all.first().map(|values| (0, &states[0], values)).into_iter().collect() } else { states.iter().enumerate().flat_map(|(state, http)| all.iter().map(move |values| (state, http, values))).collect() };
            for (state, http, values) in runs {
                let mut request = fields.clone();
                request.extend(params.iter().zip(values).map(|((name, _), value)| (format!("param_{name}"), value.clone())));
                let shown = || format!("{}{}", params.iter().zip(values).map(|((name, _), value)| format!("{name} = {value}")).collect::<Vec<_>>().join(", "), if state == 1 { "; every table empty" } else { "" });
                let (returned, rows) = match ask(http, sent, &request) {
                    Ok(answer) => answer,
                    Err(e) if e.contains("TIMEOUT_EXCEEDED") => {
                        tally.timed_out += 1;
                        continue;
                    }
                    Err(e) => {
                        tally.fail(unversioned(&e));
                        continue;
                    }
                };
                tally.ran += 1;
                if writes {
                    // A statement typed as returning nothing returns no column and no row.
                    if !returned.is_empty() || !rows.is_empty() {
                        problems.push(format!("{file}: returned {} column(s) and {} row(s), typed none (with {})", returned.len(), rows.len(), shown()));
                    }
                    continue;
                }
                // The types the result came back with are the ones the analysis read.
                if returned.len() != columns.len() {
                    problems.push(format!("{file}: returned {} column(s), typed {} (with {})", returned.len(), columns.len(), shown()));
                    continue;
                }
                let mut named = Vec::new();
                for ((name, ty), column) in returned.iter().zip(&columns) {
                    let described = column["described"].as_str().unwrap_or_default();
                    let came = types::parse(ty);
                    // The one column of a statement that returns a value goes by no name: the
                    // server's holds each constant of its expression, a parameter's value too.
                    if column["name"].as_str() != Some(name.as_str()) && analysis["value"].as_bool() != Some(true) {
                        problems.push(format!("{file}: column {name} is typed as {} (with {})", column["name"], shown()));
                    }
                    match &came {
                        Some(came) if came.to_string() == described => {}
                        _ => problems.push(format!("{file}: column {name} came back as a {ty}, typed {described} (with {})", shown())),
                    }
                    named.push(came);
                }
                match (cardinality, rows.len()) {
                    ("one", n) if n != 1 => problems.push(format!("{file}: returned {n} row(s), typed one (with {})", shown())),
                    ("at_most_one" | "maybeOne", n) if n > 1 => problems.push(format!("{file}: returned {n} rows, typed at most one (with {})", shown())),
                    _ => {}
                }
                for row in &rows {
                    for ((value, column), came) in row.iter().zip(&columns).zip(&named) {
                        let name = column["name"].as_str().unwrap_or("?");
                        let nullable = column["nullable"].as_bool().unwrap_or(false);
                        if value.is_null() {
                            if !nullable {
                                problems.push(format!("{file}: column {name} holds NULL, typed non-null (with {})", shown()));
                            }
                            continue;
                        }
                        if let Some(why) = came.as_ref().and_then(|ty| value_problem(ty, value, "")) {
                            problems.push(format!("{file}: column {name} {why} (with {})", shown()));
                        }
                        if let Some(why) = scalar_text(value).and_then(|text| contradiction(&column["type"], &text, nullable, ',')) {
                            problems.push(format!("{file}: column {name} {why} (with {})", shown()));
                        }
                    }
                }
            }
        }
    }
    report(analyzed, &tallies, timeout_ms, problems, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_expected_line_holds_the_server_s_error_by_its_name() {
        let analysis = serde_json::json!({ "diagnostics": [
            { "code": 6, "server_code": "47", "message": "Unknown expression identifier `nope` in scope SELECT nope (really). (UNKNOWN_IDENTIFIER)" },
            { "code": 6, "server_code": "62", "message": "Syntax error. Expected one of: a, b. (SYNTAX_ERROR) A ClickHouse parameter is written with its type: {id:Type}, or :id::Type." },
            { "code": 10, "message": "ClickHouse reads {s:UInt8} as a constant here, and the statement fails with each other value tried in its place, as with 2, so nothing shows that the result's types hold for every value. Write the value in the statement. (Illegal column (ILLEGAL_COLUMN))" },
            { "code": 10, "message": "Column 1 (doc) has the type JSON, and Reylo does not read a ClickHouse JSON yet" },
        ] });
        let kept = lasting(&analysis);
        let messages: Vec<&str> = kept["diagnostics"].as_array().unwrap().iter().map(|d| d["message"].as_str().unwrap()).collect();
        assert_eq!(messages, [
            "(UNKNOWN_IDENTIFIER)",
            "(SYNTAX_ERROR) A ClickHouse parameter is written with its type: {id:Type}, or :id::Type.",
            "ClickHouse reads {s:UInt8} as a constant here, and the statement fails with each other value tried in its place, as with 2, so nothing shows that the result's types hold for every value. Write the value in the statement.",
            "Column 1 (doc) has the type JSON, and Reylo does not read a ClickHouse JSON yet",
        ]);
        assert_eq!(unversioned("Code: 69. DB::Exception: Too big. (ARGUMENT_OUT_OF_BOUND) (version 26.8.40.7 (official build))"), "Code: 69. DB::Exception: Too big. (ARGUMENT_OUT_OF_BOUND)");
    }

    #[test]
    fn an_integer_s_ends_are_its_type_s() {
        assert_eq!(integer_ends(8, true), ("0".into(), "255".into()));
        assert_eq!(integer_ends(8, false), ("-128".into(), "127".into()));
        assert_eq!(integer_ends(64, true), ("0".into(), "18446744073709551615".into()));
        assert_eq!(integer_ends(64, false), ("-9223372036854775808".into(), "9223372036854775807".into()));
        assert_eq!(integer_ends(256, true).1, "115792089237316195423570985008687907853269984665640564039457584007913129639935");
    }

    #[test]
    fn a_parameter_s_values_are_written_as_its_type_reads_them() {
        let of = |name: &str| pool(&types::parse(name).unwrap());
        assert_eq!(of("UInt8").unwrap(), ["0", "1", "255"]);
        assert_eq!(of("Nullable(Bool)").unwrap(), ["true", "false", "\\N"]);
        assert_eq!(of("Decimal(4, 2)").unwrap(), ["0", "1", "-99.99", "99.99"]);
        assert_eq!(of("Decimal(2, 2)").unwrap(), ["0", "0.99", "-0.99", "0.99"]);
        assert_eq!(of("DateTime64(3, 'UTC')").unwrap(), ["00000.000", "1767268800.111", "-00001.111"]);
        assert_eq!(of("DateTime64(0)").unwrap(), ["00000", "1767268800", "-00001"]);
        assert_eq!(of("DateTime").unwrap(), ["00000", "1767268800", "4294967295"]);
        assert_eq!(of("Enum8('a' = 1, 'b' = 2)").unwrap(), ["a", "b"]);
        assert_eq!(of("Array(Tuple(UInt64, Nullable(String)))").unwrap(), ["[]", "[(1,NULL)]"]);
        assert_eq!(of("Map(String, Array(Date))").unwrap(), ["{'it\\'s':['2026-01-02']}"]);
        assert_eq!(of("JSON"), None);
    }

    #[test]
    fn a_null_outside_a_nullable_is_found_at_any_depth() {
        let found = |name: &str, json: &str| value_problem(&types::parse(name).unwrap(), &serde_json::from_str(json).unwrap(), "");
        assert_eq!(found("Array(Nullable(UInt8))", "[1, null]"), None);
        assert!(found("Array(UInt8)", "[1, null]").unwrap().contains("at [1]"));
        assert!(found("Tuple(n UInt8, s String)", r#"{"n": 1, "s": null}"#).unwrap().contains("at .s"));
        assert_eq!(found("Tuple(UInt8, Nullable(String))", "[1, null]"), None);
        assert!(found("Map(String, UInt8)", r#"[["k", null]]"#).unwrap().contains("value"));
        assert!(found("Enum8('a' = 1)", r#""b""#).unwrap().contains("no label"));
        assert_eq!(found("LowCardinality(Nullable(String))", "null"), None);
        assert_eq!(found("Variant(String, UInt8)", "null"), None);
    }

    #[test]
    fn a_script_s_statements_end_at_a_line_s_semicolon() {
        assert_eq!(script("CREATE TABLE t (\n  a UInt8\n) ENGINE = Memory;\nINSERT INTO t VALUES (1); \n\nSELECT ';' AS x"), ["CREATE TABLE t (\n  a UInt8\n) ENGINE = Memory", "INSERT INTO t VALUES (1)", "\nSELECT ';' AS x\n"]);
        assert_eq!(with_database("http://u:p@h:58123/?role=r", "d"), "http://u:p@h:58123/d?role=r");
        assert_eq!(port_of("http://u@h:8123/x"), Some(8123));
    }
}
