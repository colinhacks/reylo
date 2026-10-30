//! Postgres's regression suite: the psql scripts of `src/test/regress/sql`, in the order of `parallel_schedule`, run one after another on one session in a database of their own (`reylo_replay`, made again on the server `--url` names), since each file builds on the ones before it. The suite is read from a checkout of the server's own major version:
//!
//!   git clone --depth 1 --branch REL_18_6 https://github.com/postgres/postgres
//!   cargo run -p reylo-oracle -- --url postgres://postgres@localhost:56432/postgres --replay postgres postgres/src/test/regress
//!
//! The suite also makes roles, a tablespace and databases of its own, which outlive the run (only `reylo_replay` is dropped and made again), so `--url` names a throwaway server, and port 5432 is refused as for a case run.
//!
//! psql's meta-commands are skipped but `\set`, `\getenv` and `\c`, and its variables (`:name`, `:'name'`, `:"name"`) are written in. `COPY … FROM stdin` is fed the data below it, and `COPY … TO STDOUT` is skipped. The session takes pg_regress's settings. Files that need the suite's C library (`regress.so`) fail where they call it, which is no evidence.

use super::{Returned, Tally};
use reylo_engine::AnswerCache;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio_postgres::{Client, NoTls, SimpleQueryMessage};

const DATABASE: &str = "reylo_replay";

/// Replays the suite in `regress` on the server `url` names; whether it found no contradiction.
pub async fn run(url: &str, regress: &Path, timeout_ms: u64) -> Result<bool, String> {
    let schedule = std::fs::read_to_string(regress.join("parallel_schedule")).map_err(|e| format!("{}: {e}", regress.join("parallel_schedule").display()))?;
    let names: Vec<&str> = schedule.lines().filter_map(|l| l.trim().strip_prefix("test:")).flat_map(str::split_whitespace).collect();
    let (admin, connection) = crate::throwaway_config(url)?.connect(NoTls).await.map_err(|e| format!("{url}: {e}"))?;
    tokio::spawn(connection);
    admin.batch_execute(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)")).await.map_err(|e| e.to_string())?;
    admin.batch_execute(&format!("CREATE DATABASE {DATABASE} TEMPLATE template0 ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'")).await.map_err(|e| e.to_string())?;
    let db_url = match url.rsplit_once('/') {
        Some((server, _)) => format!("{server}/{DATABASE}"),
        None => return Err(format!("{url}: no database in the URL")),
    };
    let timeout = Duration::from_millis(timeout_ms);
    let srcdir = regress.to_string_lossy().to_string();
    let mut t = Tally::default();
    let mut client = open(&db_url, timeout_ms).await?;
    let mut vars: HashMap<String, String> = HashMap::new();
    for name in names {
        let Ok(text) = std::fs::read_to_string(regress.join("sql").join(format!("{name}.sql"))) else { continue };
        t.files += 1;
        for item in split(&text) {
            if client.is_closed() {
                client = open(&db_url, timeout_ms).await?;
            }
            match item {
                Item::Meta(line) => {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    match words.as_slice() {
                        ["\\getenv", var, env, ..] => {
                            let value = match *env {
                                "PG_ABS_SRCDIR" | "PG_ABS_BUILDDIR" => srcdir.clone(),
                                "PG_LIBDIR" => "/nonexistent".into(),
                                "PG_DLSUFFIX" => ".so".into(),
                                other => std::env::var(other).unwrap_or_default(),
                            };
                            vars.insert(var.to_string(), value);
                        }
                        // `\set name a b c`: the values joined, each a word, a 'quoted' literal or a :var.
                        ["\\set", var, values @ ..] => {
                            let value: String = values.iter().map(|v| substitute(v, &vars).trim_matches('\'').to_string()).collect();
                            vars.insert(var.to_string(), value);
                        }
                        ["\\c" | "\\connect", ..] => client = open(&db_url, timeout_ms).await?,
                        _ => {}
                    }
                }
                Item::Copy(sql, data) => {
                    use futures_util::SinkExt;
                    if let Ok(sink) = client.copy_in::<_, bytes::Bytes>(&substitute(&sql, &vars)).await {
                        let mut sink = Box::pin(sink);
                        let _ = sink.send(bytes::Bytes::from(data)).await;
                        let _ = sink.as_mut().finish().await;
                    }
                }
                Item::Sql(sql) => {
                    // Through the simple protocol, COPY's output closes the connection.
                    if sql.to_ascii_uppercase().contains("TO STDOUT") {
                        continue;
                    }
                    let sql = substitute(&sql, &vars);
                    if !super::analyzed(&sql) {
                        let _ = tokio::time::timeout(timeout, client.batch_execute(&sql)).await;
                        continue;
                    }
                    t.statements += 1;
                    let analysis = match tokio::time::timeout(timeout, reylo_engine::clients::tokio_postgres::analyze(&client, &sql, &mut AnswerCache::default())).await {
                        Ok(Ok(a)) => serde_json::to_value(&a).unwrap_or_default(),
                        _ => {
                            t.analysis_failed += 1;
                            let _ = tokio::time::timeout(timeout, client.batch_execute(&sql)).await;
                            continue;
                        }
                    };
                    if analysis["params"].as_array().is_some_and(|p| !p.is_empty()) {
                        t.with_params += 1;
                        continue;
                    }
                    match tokio::time::timeout(timeout, client.simple_query(&sql)).await {
                        Ok(Ok(messages)) => t.judge(&format!("{name}.sql"), &sql, &analysis, &returned(&messages)),
                        _ => t.failed += 1,
                    }
                }
            }
        }
    }
    drop(client);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)")).await;
    Ok(t.report())
}

/// The rows a simple query returned, and per column whether any held NULL.
fn returned(messages: &[SimpleQueryMessage]) -> Returned {
    let mut out = Returned { rows: 0, nulls: Vec::new() };
    for m in messages {
        match m {
            SimpleQueryMessage::RowDescription(columns) => out.nulls = vec![false; columns.len()],
            SimpleQueryMessage::Row(row) => {
                out.rows += 1;
                out.nulls.resize(out.nulls.len().max(row.len()), false);
                for (i, null) in out.nulls.iter_mut().enumerate() {
                    *null |= i < row.len() && row.get(i).is_none();
                }
            }
            _ => {}
        }
    }
    out
}

/// A session with pg_regress's settings.
async fn open(url: &str, timeout_ms: u64) -> Result<Client, String> {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await.map_err(|e| format!("{url}: {e}"))?;
    tokio::spawn(connection);
    let settings = format!("SET statement_timeout = {timeout_ms}; SET timezone = 'America/Los_Angeles'; SET datestyle = 'Postgres, MDY'; SET intervalstyle = 'postgres_verbose'; SET lc_messages = 'C'; SET lc_monetary = 'C'; SET lc_numeric = 'C'; SET lc_time = 'C'; SET timezone_abbreviations = 'Default'");
    let _ = client.batch_execute(&settings).await;
    Ok(client)
}

enum Item {
    Sql(String),
    /// A meta-command's line, from its backslash.
    Meta(String),
    /// `COPY … FROM stdin` and the data under it.
    Copy(String, String),
}

/// A psql script's statements and meta-commands, in order. A statement ends at a `;` outside a string, a quoted identifier, a dollar quote or a comment, or where a meta-command follows it on its line (`\gset`, `\bind`).
fn split(text: &str) -> Vec<Item> {
    let b = text.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    let mut line_start = true;
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let end_of_line = |from: usize| text[from..].find('\n').map_or(n, |e| from + e);
    while i < n {
        let c = b[i];
        if line_start && buf.trim().is_empty() {
            let j = i + text[i..].len() - text[i..].trim_start_matches([' ', '\t']).len();
            if j < n && b[j] == b'\\' {
                let e = end_of_line(j);
                out.push(Item::Meta(text[j..e].to_string()));
                buf.clear();
                i = (e + 1).min(n);
                continue;
            }
        }
        line_start = c == b'\n';
        if text[i..].starts_with("--") {
            i = end_of_line(i);
            continue;
        }
        if text[i..].starts_with("/*") {
            let mut depth = 0;
            while i < n {
                if text[i..].starts_with("/*") {
                    depth += 1;
                    i += 2;
                } else if text[i..].starts_with("*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
                if depth == 0 {
                    break;
                }
            }
            buf.push(' ');
            continue;
        }
        let quoted_end = match c {
            b'\'' => {
                let escaped = i > 0 && (b[i - 1] == b'e' || b[i - 1] == b'E');
                let mut j = i + 1;
                while j < n {
                    if b[j] == b'\'' {
                        if j + 1 < n && b[j + 1] == b'\'' {
                            j += 2;
                            continue;
                        }
                        break;
                    }
                    j += if escaped && b[j] == b'\\' { 2 } else { 1 };
                }
                Some((j + 1).min(n))
            }
            b'"' => Some(text[i + 1..].find('"').map_or(n, |e| i + e + 2)),
            b'$' if i == 0 || !word(b[i - 1]) => {
                let mut j = i + 1;
                while j < n && word(b[j]) && !(j == i + 1 && b[j].is_ascii_digit()) {
                    j += 1;
                }
                (j < n && b[j] == b'$').then(|| {
                    let tag = &text[i..=j];
                    text[j + 1..].find(tag).map_or(n, |e| j + 1 + e + tag.len())
                })
            }
            _ => None,
        };
        if let Some(e) = quoted_end {
            buf.push_str(&text[i..e]);
            i = e;
            continue;
        }
        if c == b'\\' {
            let e = end_of_line(i);
            let statement = buf.trim().to_string();
            if !statement.is_empty() {
                out.push(Item::Sql(statement));
            }
            out.push(Item::Meta(text[i..e].to_string()));
            buf.clear();
            i = e;
            continue;
        }
        if c == b';' {
            let statement = buf.trim().to_string();
            buf.clear();
            i += 1;
            if statement.is_empty() {
                continue;
            }
            let upper = statement.to_ascii_uppercase();
            if upper.starts_with("COPY") && upper.split_whitespace().collect::<Vec<_>>().windows(2).any(|w| w == ["FROM", "STDIN"]) {
                // The data starts on the next line and ends at a line of `\.`.
                let start = (end_of_line(i) + 1).min(n);
                let end = if text[start..].starts_with("\\.") { start } else { text[start..].find("\n\\.").map_or(n, |e| start + e + 1) };
                out.push(Item::Copy(statement, text[start..end].to_string()));
                i = (end_of_line(end) + 1).min(n);
                line_start = true;
                continue;
            }
            out.push(Item::Sql(statement));
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        buf.push(ch);
        i += ch.len_utf8();
    }
    let statement = buf.trim().to_string();
    if !statement.is_empty() {
        out.push(Item::Sql(statement));
    }
    out
}

/// psql's variables written in: `:name`, `:'name'` (a literal) and `:"name"` (an identifier); a `::` cast is no variable.
fn substitute(sql: &str, vars: &HashMap<String, String>) -> String {
    let mut names: Vec<&String> = vars.keys().collect();
    names.sort_by_key(|k| std::cmp::Reverse(k.len()));
    let mut out = sql.to_string();
    for name in names {
        let value = &vars[name];
        out = out.replace(&format!(":'{name}'"), &format!("'{}'", value.replace('\'', "''")));
        out = out.replace(&format!(":\"{name}\""), &format!("\"{value}\""));
        let bare = format!(":{name}");
        let mut written = String::new();
        let mut rest = out.as_str();
        while let Some(at) = rest.find(&bare) {
            let cast = at > 0 && rest.as_bytes()[at - 1] == b':';
            let longer = rest[at + bare.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_');
            written.push_str(&rest[..at]);
            written.push_str(if cast || longer { &bare } else { value });
            rest = &rest[at + bare.len()..];
        }
        written.push_str(rest);
        out = written;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_splits_into_statements_meta_commands_and_copy_data() {
        let script = "SELECT 1; -- a; comment\n\\set x 'y'\nSELECT $$a;b$$, 'c;''d', \"e;f\" /* g; */ ;\nSELECT 2 \\gset\nCOPY t FROM stdin;\n1\t2\n\\.\nSELECT :x, :'x', 1::int;\n";
        let items = split(script);
        let shown: Vec<String> = items
            .iter()
            .map(|i| match i {
                Item::Sql(s) => format!("sql {s}"),
                Item::Meta(m) => format!("meta {m}"),
                Item::Copy(s, d) => format!("copy {s} | {d}"),
            })
            .collect();
        assert_eq!(shown, ["sql SELECT 1", "meta \\set x 'y'", "sql SELECT $$a;b$$, 'c;''d', \"e;f\"", "sql SELECT 2", "meta \\gset", "copy COPY t FROM stdin | 1\t2\n", "sql SELECT :x, :'x', 1::int"]);
        let vars = HashMap::from([("x".to_string(), "y".to_string())]);
        assert_eq!(substitute("SELECT :x, :'x', :\"x\", 1::x, :xy", &vars), "SELECT y, 'y', \"y\", 1::x, :xy");
    }
}
