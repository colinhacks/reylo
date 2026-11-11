//! `reylo analyze`: each statement's type descriptor (spec/index.ts), one JSON line per
//! statement, read from the database the project names, as the type check reads it.

use crate::project::{self, Project};
use reylo_engine_core::api::{utf16_offset, Analysis};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::types::{Cardinality, DialectVersion, Diagnostic, Kind, RowsType, StatementDescriptor};
use reylo_engine_native::Failure;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "Usage: reylo analyze <file.sql | directory | -> ... [--url <url>]
       reylo analyze --sql <statement> [--url <url>]

Prints each statement's type descriptor as one line of JSON, with the file it was read from.
A directory is searched for .sql files, leaving out node_modules, dist, build, out, target,
coverage, hidden directories and what git ignores; - reads one statement from standard input.
The database is --url, else the project file's direct_url, else DATABASE_URL, from the
environment or the .env files; a relative SQLite path in --url is read from the working
directory, one in the project's URL from the project's root. A statement the engine cannot
analyze is printed with one diagnostic that says why. Exits with 1 when any statement has a
diagnostic, and with 2 when the database cannot be reached or the statements cannot be read.";

/// The directories a directory's search leaves out: dependencies and build output.
const SKIPPED: &[&str] = &["node_modules", "dist", "build", "out", "target", "coverage"];

/// One statement to analyze, and the file it was read from.
struct Input {
    file: Option<String>,
    sql: String,
}

/// One database's statements, over one connection.
enum Session {
    Postgres(reylo_engine_native::postgres::Session),
    Mysql(reylo_engine_native::mysql::Session),
    Sqlite(reylo_engine_native::sqlite::Session),
}

impl Session {
    /// `base` is the directory a relative SQLite path is read from.
    async fn open(dialect: &str, url: &str, base: &Path) -> Result<Session, Failure> {
        Ok(match dialect {
            "postgres" => Session::Postgres(reylo_engine_native::postgres::Session::open(url).await?),
            "mysql" => Session::Mysql(reylo_engine_native::mysql::Session::open(url).await?),
            _ => Session::Sqlite(reylo_engine_native::sqlite::Session::open(&project::sqlite_path(url, base).map_err(Failure::Error)?)?),
        })
    }

    fn version(&self) -> &str {
        match self {
            Session::Postgres(s) => &s.version,
            Session::Mysql(s) => &s.version,
            Session::Sqlite(s) => &s.version,
        }
    }

    async fn analyze(&mut self, sql: &str) -> Result<Analysis, Failure> {
        match self {
            Session::Postgres(s) => s.analyze(sql).await,
            Session::Mysql(s) => s.analyze(sql).await,
            Session::Sqlite(s) => s.analyze(sql),
        }
    }
}

pub fn run(args: &[String]) -> i32 {
    let mut url: Option<String> = None;
    let mut sql: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return 0;
            }
            "--url" | "--sql" => match rest.next() {
                Some(value) if arg == "--url" => url = Some(value.clone()),
                Some(value) => sql = Some(value.clone()),
                None => return usage(&format!("{arg} takes a value")),
            },
            _ if arg.starts_with("--url=") => url = Some(arg["--url=".len()..].to_string()),
            _ if arg.starts_with("--sql=") => sql = Some(arg["--sql=".len()..].to_string()),
            _ if arg.starts_with('-') && arg != "-" => return usage(&format!("unknown option {arg}")),
            _ => paths.push(arg.clone()),
        }
    }
    if sql.is_none() && paths.is_empty() {
        return usage("name a .sql file, a directory, - for standard input, or --sql");
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let env: HashMap<String, String> = std::env::vars().collect();
    let project = match Project::resolve(&cwd, env) {
        Ok(project) => project,
        Err(message) => return fail(&message),
    };
    if let Some(notice) = project.notice() {
        eprintln!("reylo analyze: {notice}");
    }
    let mut inputs: Vec<Input> = sql.map(|sql| Input { file: None, sql }).into_iter().collect();
    for path in &paths {
        match read_inputs(path, &cwd) {
            Ok(found) => inputs.extend(found),
            Err(message) => return fail(&message),
        }
    }
    if inputs.is_empty() {
        return fail(&format!("no .sql file under {}", paths.join(", ")));
    }
    let given = url.is_some();
    let url = match url.map(Ok).unwrap_or_else(|| project.type_check_url().map(|u| u.unwrap_or_default())) {
        Ok(url) if !url.is_empty() => url,
        Ok(_) => return fail(&project.missing_url()),
        Err(message) => return fail(&message),
    };
    let Some(dialect) = project::dialect_of(&url) else {
        return fail(&format!("Cannot tell the database from the URL ({}): reylo reads postgres://, postgresql://, mysql://, file:, sqlite: and duckdb: URLs.", if given { "--url" } else { "the project's" }));
    };
    if dialect == "duckdb" {
        return fail("A DuckDB database runs inside the app, and the type check reads it through the project's @duckdb/node-api: reylo analyze cannot open one yet.");
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    // A relative SQLite path in a URL typed on the command line is the shell's, from the working
    // directory; one in the project's URL is from the project's root, as the type check reads it.
    let base = if given { cwd.clone() } else { project.root.clone() };
    runtime.block_on(analyze_all(dialect, url, given, &base, &project, inputs))
}

async fn analyze_all(dialect: &str, url: String, given: bool, base: &Path, project: &Project, inputs: Vec<Input>) -> i32 {
    let mut session = match Session::open(dialect, &url, base).await {
        Ok(session) => session,
        // A command in `direct_url` that makes a short-lived token is run again once the token is
        // refused, as the type check runs it again.
        Err(Failure::Refused(_)) if !given && project.url_runs_command() => match project.type_check_url() {
            Ok(Some(fresh)) => match Session::open(dialect, &fresh, base).await {
                Ok(session) => session,
                Err(failure) => return fail(&failure.to_string()),
            },
            Ok(None) => return fail(&project.missing_url()),
            Err(message) => return fail(&message),
        },
        Err(failure) => return fail(&failure.to_string()),
    };
    let version = DialectVersion { name: dialect.to_string(), version: session.version().to_string() };
    let engine_dialect = match dialect {
        "postgres" => Dialect::Postgres,
        "mysql" => Dialect::Mysql,
        _ => Dialect::Sqlite,
    };
    let mut flagged = false;
    for input in inputs {
        let descriptor = match session.analyze(&input.sql).await {
            Ok(analysis) => {
                let mut descriptor = analysis.descriptor(&input.sql, version.clone());
                if project.strict_params() && analysis.diagnostics.is_empty() {
                    descriptor.diagnostics.extend(untyped_params(&input.sql, &analysis, engine_dialect, project));
                }
                descriptor
            }
            // The connection is gone, or never was the database's: no later statement can be read.
            Err(failure @ (Failure::Unreachable(_) | Failure::Refused(_) | Failure::NotMysql(_))) => {
                return fail(&match &input.file {
                    Some(file) => format!("{file}: {failure}"),
                    None => failure.to_string(),
                })
            }
            // This statement alone could not be analyzed: its line says why, over the whole
            // statement, and the run goes on.
            Err(Failure::Error(message)) => StatementDescriptor {
                schema: reylo_engine_core::types::SCHEMA.to_string(),
                dialect: version.clone(),
                sql: input.sql.clone(),
                kind: None,
                comment: None,
                params: Vec::new(),
                result: RowsType { cardinality: Cardinality::Many, columns: Vec::new(), unique: Vec::new(), order: Vec::new() }.into(),
                diagnostics: vec![Diagnostic { message, start: 0, end: utf16_offset(&input.sql, input.sql.len()), code: None }],
            },
        };
        flagged |= !descriptor.diagnostics.is_empty();
        let mut line = Map::new();
        if let Some(file) = input.file {
            line.insert("file".into(), Value::String(file));
        }
        if let Ok(Value::Object(fields)) = serde_json::to_value(&descriptor) {
            line.extend(fields);
        }
        println!("{}", Value::Object(line));
    }
    i32::from(flagged)
}

/// A parameter nothing in the statement types is an error at its first use, unless the project
/// file says `allow_untyped_params: true`, as the type check says it (src/module-types.ts). A statement
/// the engine refused already was not analyzed to its end, and is not asked.
fn untyped_params(sql: &str, analysis: &Analysis, dialect: Dialect, project: &Project) -> Vec<Diagnostic> {
    // A MySQL session reads its strings by its sql_mode, which the lexer alone does not know: the
    // placeholders are the ones the analysis read.
    let placeholders: Vec<(String, usize, usize)> = match dialect {
        Dialect::Mysql => analysis.rewritten.spans.iter().zip(&analysis.rewritten.order).map(|(&(start, end, _, _), name)| (name.clone(), start, end)).collect(),
        _ => reylo_engine_core::lex::lex(sql, dialect).map(|l| l.placeholders.into_iter().map(|p| (p.name, p.start, p.end)).collect()).unwrap_or_default(),
    };
    let file = project.file.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| ".reylo.yaml".into());
    analysis
        .params
        .iter()
        .filter(|p| matches!(p.ty.kind, Kind::Unknown))
        .map(|p| {
            let at = placeholders.iter().find(|(name, _, _)| *name == p.name);
            // A MySQL placeholder's span takes in the `::type` the engine rewrote, which is no part of its name.
            let span = at.map_or_else(|| format!(":{}", p.name), |&(_, start, end)| sql.get(start..end).unwrap_or_default().to_string());
            let written = span.split("::").next().unwrap_or_default().trim_end().to_string();
            let (start, end) = at.map_or((0, 1), |&(_, start, _)| (utf16_offset(sql, start), utf16_offset(sql, start + written.len())));
            Diagnostic { message: format!("Nothing in the statement gives the parameter {written} a type; cast it, as in {written}::text, or set allow_untyped_params: true in {file}."), start, end, code: None }
        })
        .collect()
}

/// The statements a path names: a file's text, every `.sql` file under a directory, or standard input.
fn read_inputs(path: &str, cwd: &Path) -> Result<Vec<Input>, String> {
    if path == "-" {
        let mut sql = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut sql).map_err(|e| format!("cannot read standard input: {e}"))?;
        return Ok(vec![Input { file: None, sql }]);
    }
    let full = cwd.join(path);
    let files = if full.is_dir() {
        let mut found = Vec::new();
        sql_files(&full, &mut found);
        let ignored = git_ignored(&full, &found);
        found.retain(|f| !ignored.contains(f));
        found.sort();
        found
    } else if full.is_file() {
        vec![full]
    } else {
        return Err(format!("{path} does not exist"));
    };
    files
        .into_iter()
        .map(|file| {
            let sql = std::fs::read_to_string(&file).map_err(|e| format!("cannot read {}: {e}", shown(cwd, &file)))?;
            Ok(Input { file: Some(shown(cwd, &file)), sql })
        })
        .collect()
}

/// Every `.sql` file under `dir`, leaving out the directories [`SKIPPED`] names and hidden ones but
/// `.server`, whose queries a framework keeps there.
fn sql_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if !SKIPPED.contains(&name.as_str()) && (!name.starts_with('.') || name == ".server") {
                sql_files(&path, out);
            }
        } else if kind.is_file() && path.extension().is_some_and(|x| x.eq_ignore_ascii_case("sql")) {
            out.push(path);
        }
    }
}

/// The files among `files` that git ignores: none where git is missing or `dir` is in no repository.
fn git_ignored(dir: &Path, files: &[PathBuf]) -> Vec<PathBuf> {
    use std::io::Write;
    if files.is_empty() {
        return Vec::new();
    }
    let Ok(mut child) = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["check-ignore", "--stdin", "-z"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let list: Vec<u8> = files.iter().flat_map(|f| f.to_string_lossy().into_owned().into_bytes().into_iter().chain([0])).collect();
    // The list is written while git's answer is read: git writes each ignored path as it reads it,
    // and stops reading once a full pipe stops its writes.
    let writer = child.stdin.take().map(|mut stdin| std::thread::spawn(move || drop(stdin.write_all(&list))));
    let Ok(out) = child.wait_with_output() else { return Vec::new() };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned())).collect()
}

/// A path from the working directory, written with `/`, as the type check writes it.
fn shown(cwd: &Path, path: &Path) -> String {
    match path.strip_prefix(cwd) {
        Ok(rest) => rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn usage(message: &str) -> i32 {
    eprintln!("reylo analyze: {message}\n\n{USAGE}");
    2
}

fn fail(message: &str) -> i32 {
    eprintln!("reylo analyze: {message}");
    2
}
