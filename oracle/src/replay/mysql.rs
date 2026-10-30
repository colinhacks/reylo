//! MySQL's own tests: the mysqltest scripts of `mysql-test/t`, each on a fresh database `test`, their SQL run in order on one session. The suite is read from a checkout at the tag of the server that runs it, since a newer suite's tests can crash an older server:
//!
//!   git clone --depth 1 --branch mysql-8.4.11 --filter=blob:none --sparse https://github.com/mysql/mysql-server && git -C mysql-server sparse-checkout set mysql-test
//!   cargo run -p reylo-oracle -- --replay mysql --mysqld /opt/homebrew/opt/mysql@8.4/bin/mysqld mysql-server/mysql-test/t
//!
//! The runner owns its server, which it starts from `--mysqld` on a data directory of its own: a test may lock a user out, change how logins authenticate, or crash the server, and the runner then puts the data directory back as it was made and starts the server again. It connects as a user of its own, at both `localhost` and `127.0.0.1`, and leaves out the statements that would end its session or its server or change its user. mysqltest's commands are skipped but `send` and `query`, whose SQL runs; a script that needs a debug build, a plugin, a component, binary logging, NDB or SSL is skipped whole; the sessions a script switches between are one session.

use super::{Returned, Tally};
use mysql_async::prelude::Queryable;
use mysql_async::{Conn, Opts, OptsBuilder};
use reylo_engine::AnswerCache;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const USER: &str = "reylo_replay";

/// mysqltest's commands (`client/mysqltest.cc`, `command_names[]`): a statement whose first word is one of them is the tool's, not SQL.
const COMMANDS: &[&str] = &[
    "connection", "query", "connect", "sleep", "inc", "dec", "source", "disconnect", "let", "echo", "expr", "while", "end", "save_master_pos", "sync_with_master", "sync_slave_with_master", "error", "send", "reap", "dirty_close", "replace_result", "replace_column", "ping", "eval", "enable_query_log", "disable_query_log", "enable_result_log", "disable_result_log", "enable_connect_log", "disable_connect_log", "wait_for_slave_to_stop", "enable_warnings", "disable_warnings", "enable_info", "disable_info", "enable_session_track_info", "disable_session_track_info", "enable_metadata", "disable_metadata", "enable_async_client", "disable_async_client", "exec", "execw", "exec_in_background", "delimiter", "disable_abort_on_error", "enable_abort_on_error", "vertical_results", "horizontal_results", "query_vertical", "query_horizontal", "sorted_result", "partially_sorted_result", "lowercase_result", "skip_if_hypergraph", "start_timer", "end_timer", "character_set", "disable_ps_protocol", "enable_ps_protocol", "if", "disable_testcase", "enable_testcase", "replace_regex", "replace_numeric_round", "remove_file", "file_exists", "write_file", "copy_file", "perl", "die", "assert", "exit", "skip", "chmod", "append_file", "cat_file", "diff_files", "send_quit", "change_user", "mkdir", "rmdir", "force-rmdir", "force-cpdir", "list_files", "list_files_write_file", "list_files_append_file", "send_shutdown", "shutdown_server", "result_format", "move_file", "remove_files_wildcard", "copy_files_wildcard", "send_eval", "output", "reset_connection", "query_attributes",
];

/// An include that says the script needs what this server or this runner lacks.
const NEEDS: &[&str] = &["have_debug", "have_ndb", "have_binlog", "not_valgrind", "have_component", "plugin", "have_util_", "ssl", "restart_mysqld", "have_ipv6", "have_hypergraph", "have_grep", "have_case_insensitive", "have_lowercase"];

/// Replays the scripts under `paths` on a server of the runner's own, started from `mysqld`; whether it found no contradiction.
pub async fn run(mysqld: &Path, paths: &[PathBuf], timeout_ms: u64) -> Result<bool, String> {
    let server = Owned::make(mysqld).await?;
    let outcome = replay(&server, paths, timeout_ms).await;
    server.stop();
    let _ = std::fs::remove_dir_all(&server.dir);
    outcome
}

async fn replay(server: &Owned, paths: &[PathBuf], timeout_ms: u64) -> Result<bool, String> {
    let timeout = Duration::from_millis(timeout_ms);
    let mut t = Tally::default();
    let mut restarts = 0usize;
    for file in super::files(paths, "test") {
        t.files += 1;
        let Ok(text) = std::fs::read_to_string(&file) else {
            t.files_skipped += 1;
            continue;
        };
        let records = parse(&text);
        if records.iter().any(|r| matches!(r, Record::Command(c, arg) if c == "source" && NEEDS.iter().any(|n| arg.contains(n)))) {
            t.files_skipped += 1;
            continue;
        }
        let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        // A fresh database for the script, and a session in it; a server a script left unreachable is made again.
        let mut session = None;
        for _ in 0..2 {
            match server.fresh_session().await {
                Ok(s) => {
                    session = Some(s);
                    break;
                }
                Err(_) => {
                    restarts += 1;
                    server.restore()?;
                }
            }
        }
        let Some((mut admin, mut conn)) = session else { return Err(format!("{name}: the server cannot be reached after it was made again")) };
        for record in records {
            let sql = match record {
                Record::Sql(sql) => sql,
                Record::Command(c, arg) if (c == "send" || c == "query") && !arg.trim().is_empty() => arg,
                Record::Command(..) => continue,
            };
            if denied(&sql) {
                continue;
            }
            if !super::analyzed(&sql) {
                if tokio::time::timeout(timeout, conn.query_drop(&sql)).await.is_err() {
                    let _ = admin.query_drop(format!("KILL {}", conn.id())).await;
                    match server.session(Some("test")).await {
                        Ok(next) => conn = next,
                        Err(_) => break,
                    }
                }
                continue;
            }
            t.statements += 1;
            let analysis = match tokio::time::timeout(timeout, reylo_engine::clients::mysql_async::analyze(&mut conn, &sql, &mut AnswerCache::default())).await {
                Ok(Ok(a)) => serde_json::to_value(&a).unwrap_or_default(),
                outcome => {
                    t.analysis_failed += 1;
                    if outcome.is_err() {
                        let _ = admin.query_drop(format!("KILL {}", conn.id())).await;
                        match server.session(Some("test")).await {
                            Ok(next) => conn = next,
                            Err(_) => break,
                        }
                    } else {
                        let _ = tokio::time::timeout(timeout, conn.query_drop(&sql)).await;
                    }
                    continue;
                }
            };
            if analysis["params"].as_array().is_some_and(|p| !p.is_empty()) {
                t.with_params += 1;
                continue;
            }
            match tokio::time::timeout(timeout, returned(&mut conn, &sql)).await {
                Ok(Ok(r)) => t.judge(&name, &sql, &analysis, &r),
                Ok(Err(_)) => t.failed += 1,
                Err(_) => {
                    t.failed += 1;
                    let _ = admin.query_drop(format!("KILL {}", conn.id())).await;
                    match server.session(Some("test")).await {
                        Ok(next) => conn = next,
                        Err(_) => break,
                    }
                }
            }
        }
        let _ = conn.disconnect().await;
        let _ = admin.disconnect().await;
    }
    println!("{restarts} time(s) the server was made again after a script left it unreachable");
    Ok(t.report())
}

/// The rows a statement returned, and per column whether any held NULL; a second result (a statement that returns several) is not this statement's.
async fn returned(conn: &mut Conn, sql: &str) -> Result<Returned, mysql_async::Error> {
    let mut result = conn.query_iter(sql).await?;
    let mut out = Returned { rows: 0, nulls: vec![false; result.columns_ref().len()] };
    let rows: Vec<mysql_async::Row> = result.collect().await?;
    for row in &rows {
        out.rows += 1;
        for (i, null) in out.nulls.iter_mut().enumerate() {
            *null |= matches!(row.as_ref(i), Some(mysql_async::Value::NULL));
        }
    }
    let _ = result.drop_result().await;
    Ok(out)
}

/// A statement the runner leaves out: one that would end its session or its server, or change its user or the tables that grant it.
fn denied(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let first = words.first().copied().unwrap_or("");
    let global = matches!(words.as_slice(), ["set", scope, ..] if scope.starts_with("global") || scope.starts_with("persist") || scope.starts_with("@@global"));
    let grants = ["mysql.user", "mysql.db", "mysql.global_grants", "mysql.tables_priv", "mysql.proxies_priv"].iter().any(|t| lower.contains(t)) && matches!(first, "insert" | "update" | "delete" | "replace" | "truncate");
    matches!(first, "shutdown" | "kill" | "restart" | "clone")
        || (first == "alter" && words.get(1) == Some(&"instance"))
        || (global && ["read_only", "super_read_only", "offline_mode", "max_connections", "innodb_fast_shutdown"].iter().any(|v| lower.contains(v)))
        || lower.contains(USER)
        || grants
}

enum Record {
    Sql(String),
    /// A mysqltest command and its argument.
    Command(String, String),
}

/// A script's records as mysqltest reads them: a `#` line is a comment and a `--` line a command; anything else runs up to the current delimiter, and is a command when its first word is one of [`COMMANDS`]. The bodies of `perl`, `write_file` and `append_file` are skipped.
fn parse(text: &str) -> Vec<Record> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut delimiter = ";".to_string();
    let mut buf: Vec<&str> = Vec::new();
    let mut i = 0;
    let skip_block = |i: &mut usize| {
        while *i < lines.len() && !matches!(lines[*i].trim(), "EOF" | "END_OF_PROCEDURE") {
            *i += 1;
        }
        *i += 1;
    };
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        i += 1;
        if buf.is_empty() {
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(command) = trimmed.strip_prefix("--") {
                let (word, arg) = command.split_once(char::is_whitespace).unwrap_or((command, ""));
                if matches!(word, "perl" | "write_file" | "append_file") {
                    skip_block(&mut i);
                }
                if word == "delimiter" && !arg.trim().is_empty() {
                    delimiter = arg.trim().to_string();
                }
                out.push(Record::Command(word.to_string(), arg.to_string()));
                continue;
            }
            if trimmed.starts_with('}') || trimmed == "{" {
                continue;
            }
            let word = trimmed.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).next().unwrap_or("").to_ascii_lowercase();
            // `if (…)` and `while (…)`, whose block's SQL runs once here; the braces are skipped above.
            if matches!(word.as_str(), "if" | "while") {
                out.push(Record::Command(word, trimmed.to_string()));
                continue;
            }
        }
        buf.push(line);
        let joined = buf.join("\n");
        let Some(statement) = joined.trim_end().strip_suffix(delimiter.as_str()) else { continue };
        let statement = statement.trim().to_string();
        buf.clear();
        let (word, arg) = statement.split_once(char::is_whitespace).unwrap_or((statement.as_str(), ""));
        let word = word.to_ascii_lowercase();
        if COMMANDS.contains(&word.as_str()) {
            if matches!(word.as_str(), "perl" | "write_file" | "append_file") {
                skip_block(&mut i);
            }
            if word == "delimiter" && !arg.trim().is_empty() {
                delimiter = arg.trim().to_string();
            }
            out.push(Record::Command(word, arg.to_string()));
        } else if !statement.is_empty() {
            out.push(Record::Sql(statement));
        }
    }
    out
}

/// The server the runner owns: its data directory as made (`pristine`), the copy it runs on, and its port.
struct Owned {
    mysqld: PathBuf,
    dir: PathBuf,
    port: u16,
}

impl Owned {
    /// A data directory made with the runner's user, kept as made, and a server started on a copy of it.
    async fn make(mysqld: &Path) -> Result<Owned, String> {
        let dir = std::env::temp_dir().join(format!("reylo-replay-mysql-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_err(|e| e.to_string())?.port();
        let server = Owned { mysqld: mysqld.to_path_buf(), dir, port };
        let made = Command::new(mysqld).arg("--no-defaults").arg("--initialize-insecure").arg(format!("--datadir={}", server.dir.join("pristine").display())).arg(format!("--user={}", std::env::var("USER").unwrap_or_default())).output().map_err(|e| format!("{}: {e}", mysqld.display()))?;
        if !made.status.success() {
            return Err(format!("{} --initialize-insecure: {}", mysqld.display(), String::from_utf8_lossy(&made.stderr)));
        }
        server.start("pristine")?;
        let root = format!("mysql://root@127.0.0.1:{}", server.port);
        let users = format!("CREATE USER '{USER}'@'localhost', '{USER}'@'127.0.0.1'; GRANT ALL ON *.* TO '{USER}'@'localhost' WITH GRANT OPTION; GRANT ALL ON *.* TO '{USER}'@'127.0.0.1' WITH GRANT OPTION");
        let made = async {
            let mut conn = Conn::new(Opts::from_url(&root).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
            conn.query_drop(users).await.map_err(|e| e.to_string())?;
            conn.disconnect().await.map_err(|e| e.to_string())
        }
        .await;
        server.stop();
        made?;
        server.restore()?;
        Ok(server)
    }

    fn start(&self, data: &str) -> Result<(), String> {
        let d = &self.dir;
        let started = Command::new(&self.mysqld)
            .arg("--no-defaults")
            .arg("--daemonize")
            .arg(format!("--datadir={}", d.join(data).display()))
            .arg(format!("--socket={}", d.join("sock").display()))
            .arg(format!("--port={}", self.port))
            .arg("--bind-address=127.0.0.1")
            .arg("--mysqlx=OFF")
            .arg(format!("--pid-file={}", d.join("pid").display()))
            .arg(format!("--log-error={}", d.join("err.log").display()))
            .arg(format!("--user={}", std::env::var("USER").unwrap_or_default()))
            .output()
            .map_err(|e| e.to_string())?;
        if !started.status.success() {
            return Err(format!("mysqld did not start: {}", std::fs::read_to_string(d.join("err.log")).unwrap_or_default().lines().rev().take(5).collect::<Vec<_>>().join(" | ")));
        }
        for _ in 0..100 {
            if std::net::TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("mysqld started and never listened".into())
    }

    fn stop(&self) {
        if let Ok(pid) = std::fs::read_to_string(self.dir.join("pid")) {
            let kill = |args: &[&str]| Command::new("kill").args(args).arg(pid.trim()).stderr(std::process::Stdio::null()).status();
            let _ = kill(&[]);
            for _ in 0..300 {
                if kill(&["-0"]).is_ok_and(|s| !s.success()) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            // Only a server still running is killed: once it is gone, its pid may be another process's.
            if kill(&["-0"]).is_ok_and(|s| s.success()) {
                let _ = kill(&["-9"]);
            }
            let _ = std::fs::remove_file(self.dir.join("pid"));
        }
    }

    /// The server stopped, its data directory put back as it was made, and started again.
    fn restore(&self) -> Result<(), String> {
        self.stop();
        let data = self.dir.join("data");
        let _ = std::fs::remove_dir_all(&data);
        let copied = Command::new("cp").arg("-R").arg(self.dir.join("pristine")).arg(&data).status().map_err(|e| e.to_string())?;
        if !copied.success() {
            return Err("cannot copy the data directory as it was made".into());
        }
        self.start("data")
    }

    async fn session(&self, db: Option<&str>) -> Result<Conn, mysql_async::Error> {
        let opts = OptsBuilder::default().ip_or_hostname("127.0.0.1").tcp_port(self.port).user(Some(USER)).db_name(db).prefer_socket(false).stmt_cache_size(0);
        Conn::new(opts).await
    }

    /// An administrative session, and a session in a fresh database `test` with no other database of a script's left.
    async fn fresh_session(&self) -> Result<(Conn, Conn), mysql_async::Error> {
        let mut admin = self.session(None).await?;
        let kept = ["mysql", "information_schema", "performance_schema", "sys"];
        let databases: Vec<String> = admin.query("SELECT schema_name FROM information_schema.schemata").await?;
        for db in databases.iter().filter(|d| !kept.contains(&d.as_str())) {
            admin.query_drop(format!("DROP DATABASE IF EXISTS `{}`", db.replace('`', "``"))).await?;
        }
        admin.query_drop("CREATE DATABASE test").await?;
        let conn = self.session(Some("test")).await?;
        Ok((admin, conn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_reads_as_mysqltest_reads_it() {
        let script = "# a comment\n--source include/have_innodb.inc\nCREATE TABLE t (a INT);\nSELECT a\n  FROM t;\n--error ER_NO_SUCH_TABLE\nSELECT * FROM nope;\nlet $x = 1;\ndelimiter //;\nCREATE PROCEDURE p() BEGIN SELECT 1; END//\ndelimiter ;//\n--perl\nprint 1;\nEOF\nif ($x)\n{\n  SELECT 2;\n}\n";
        let shown: Vec<String> = parse(script)
            .into_iter()
            .map(|r| match r {
                Record::Sql(s) => format!("sql {s}"),
                Record::Command(c, a) => format!("{c} {a}").trim().to_string(),
            })
            .collect();
        assert_eq!(shown, ["source include/have_innodb.inc", "sql CREATE TABLE t (a INT)", "sql SELECT a\n  FROM t", "error ER_NO_SUCH_TABLE", "sql SELECT * FROM nope", "let $x = 1", "delimiter //", "sql CREATE PROCEDURE p() BEGIN SELECT 1; END", "delimiter ;", "perl", "if if ($x)", "sql SELECT 2"]);
        assert!(denied("KILL 4") && denied("set global read_only = 1") && denied("DROP USER reylo_replay@localhost") && denied("delete from mysql.user where user = ''"));
        assert!(!denied("select * from mysql.user") && !denied("set global sql_mode = ''"));
    }
}
