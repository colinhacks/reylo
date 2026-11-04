//! `reylo`, one program for every database: the analysis engine a language's frontend talks to over
//! standard input and output (`reylo engine <database>`, one JSON request per line, one answer per
//! line), each statement's type descriptor on the command line (`reylo analyze`), and the
//! migrations (`reylo migrate`, `reylo run`). npm's `reylo` command runs it (src/cli.ts), but for
//! `reylo play`, the playground's server, which renders TypeScript and so runs in JavaScript.

mod analyze;
mod project;

use project::Project;
use reylo_migrate::Options;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

const USAGE: &str = "Usage: reylo <command>

  analyze <paths> [--url]   Print each statement's type descriptor, one line of JSON per
                            statement, read from the project's database
  migrate <command>         Check and apply migrations (reylo migrate --help)
  run \"<sql>\"… [--file <path>]…
                            Run SQL on the database as written; records nothing
  play [--port] [--no-open] Serve the playground on 127.0.0.1, against the project's database
                            (from reylo's npm package: npx reylo play)
  engine <database> [call]  Serve the analysis engine of postgres, mysql, sqlite or duckdb over
                            standard input and output, one JSON request per line; with call,
                            answer the one request on standard input
  --version                 Print the version";

const RUN_USAGE: &str = "Usage: reylo run \"<sql>\"… [--file <path>]…

Runs SQL on the database as written, outside any migration's history: inline text, and the files
--file names (-f), in the order given. The database is migrate.url, else the type check's
connection; --url names another.";

const MIGRATE_USAGE: &str = "Usage: reylo migrate <command>

  init [--schema <name>…]   Write schema.sql and migrations/init.sql from the database's schema
                            (Postgres: the public schema unless named), and record init.sql
  create <name>             Write the migration that takes the migrations to schema.sql, checked in
                            a shadow database, named with the current UTC time
  apply [--record <file>…]  Check the database against the migrations in shadow databases, then
                            apply the pending ones. Refuses on any disagreement and runs nothing
                            then. --record records a migration without running it, where the
                            database already has its change and it changes no data (a fix made
                            by hand, written down afterwards)
  apply --reset             Drop everything, apply every migration, then run migrate.post (loses
                            data)
  check                     What apply checks, and the migrations against schema.sql; runs nothing
  restore <file>            Write the text this database ran for a migration back to its file
  repair                    When the database, schema.sql and the migrations agree, rewrite the
                            history to the migration files; never touches the schema or data
  rebase [files…] [--base <branch>]
                            Rename this branch's migrations to times after the newest. Bare, with
                            a linear history, it puts them in the order of migrations/lock.yaml,
                            resolving a merge conflict in it; otherwise it takes the ones the base
                            branch (migrate.main_branch, else git's origin/HEAD) does not have

reylo run \"<sql>\"…          Run SQL on the database as written; --file <path> for a file (a seed)

Settings, in .reylo.yaml:
  migrate:
    dir: migrations          # default
    schema: schema.sql       # default
    url: ${DATABASE_URL}     # default: the type check's connection
    shadow_url: ${SHADOW_URL}  # a server for shadow databases, where the target's refuses create database
    history: loose           # default; linear refuses a migration older than one applied, and
                             # keeps migrations/lock.yaml, the order they run in
    strict_comments: true    # default; false leaves comments out of the comparison
    main_branch: main        # for a bare rebase; default: git's origin/HEAD
    post: reylo run --file seed.sql   # run after apply --reset

Flags override them for a run: --url, --dir, --schema-file, --base.";

/// The arguments after a command: flags with values, lists, switches and the rest.
#[derive(Default)]
struct Args {
    values: HashMap<String, String>,
    schemas: Vec<String>,
    files: Vec<String>,
    records: Vec<String>,
    switches: Vec<String>,
    rest: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut k = 0;
    while k < args.len() {
        let a = &args[k];
        let mut value = || -> Result<String, String> {
            k += 1;
            args.get(k).cloned().ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_str() {
            "--url" | "--dir" | "--schema-file" | "--base" => {
                let v = value()?;
                out.values.insert(a[2..].to_string(), v);
            }
            "--schema" => out.schemas.push(value()?),
            "--file" | "-f" => out.files.push(value()?),
            "--record" => out.records.push(value()?),
            s if s.starts_with("--") => out.switches.push(s[2..].to_string()),
            _ => out.rest.push(a.clone()),
        }
        k += 1;
    }
    Ok(out)
}

/// The project's `migrate` settings, flags over the file. `require` refuses a project with no
/// `migrate` section, whose presence opts the project in.
fn settings(args: &Args, require: bool) -> Result<Options, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let project = Project::resolve(&cwd, std::env::vars().collect())?;
    let m = project.config.migrate.clone();
    if m.is_none() && require && !args.values.contains_key("url") {
        let file = project.file.as_deref().map(|f| f.file_name().unwrap_or_default().to_string_lossy().into_owned()).unwrap_or_else(|| ".reylo.yaml".into());
        return Err(format!("no migrate section in {file}: add one (migrate: {{}}) to use reylo migrate"));
    }
    let m = m.unwrap_or_default();
    let url = match (args.values.get("url"), &m.url) {
        (Some(url), _) => url.clone(),
        (None, Some(url)) => project.expand("migrate.url", url)?,
        (None, None) => project.type_check_url()?.unwrap_or_default(),
    };
    if url.is_empty() {
        return Err("no database: set migrate.url or direct_url in .reylo.yaml, or DATABASE_URL".into());
    }
    reylo_migrate::Dialect::of_url(&url).map_err(|e| e.0)?;
    let shadow_url = m.shadow_url.as_deref().map(|u| project.expand("migrate.shadow_url", u)).transpose()?;
    let place = |flag: &str, key: &Option<String>, default: &str| -> PathBuf {
        match args.values.get(flag) {
            Some(v) => cwd.join(v),
            None => project.root.join(key.as_deref().unwrap_or(default)),
        }
    };
    Ok(Options {
        url,
        shadow_url,
        dir: place("dir", &m.dir, "migrations"),
        schema: place("schema-file", &m.schema, "schema.sql"),
        linear: m.history == Some(reylo_engine_core::config::History::Linear),
        comments: m.strict_comments != Some(false),
        main_branch: args.values.get("base").cloned().or(m.main_branch.clone()),
        post: m.post.clone(),
        root: project.root.clone(),
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime")
}

fn print(line: &str) {
    println!("{line}");
}

fn migrate_command(args: &[String]) -> Result<i32, String> {
    let Some(command) = args.first() else {
        println!("{MIGRATE_USAGE}");
        return Ok(0);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{MIGRATE_USAGE}");
        return Ok(0);
    }
    let parsed = parse(&args[1..])?;
    let opts = settings(&parsed, true)?;
    let has = |s: &str| parsed.switches.iter().any(|x| x == s);
    let mut log = print;
    let rt = runtime();
    let result = rt.block_on(async {
        match command.as_str() {
            "apply" if has("reset") => reylo_migrate::reset(&opts, &mut log).await,
            // `--dry-run` was the check's flag before it was a command of its own.
            "check" => reylo_migrate::check_command(&opts, &mut log).await,
            "apply" if has("dry-run") => reylo_migrate::check_command(&opts, &mut log).await,
            "apply" => reylo_migrate::migrate(&opts, &parsed.records, &mut log).await,
            "restore" => match parsed.rest.first() {
                Some(name) => reylo_migrate::restore(&opts, name, &mut log).await,
                None => Err(reylo_migrate::Error("name the migration to restore: reylo migrate restore <file>".into())),
            },
            "init" => reylo_migrate::init(&opts, &parsed.schemas, &mut log).await,
            "create" => reylo_migrate::create(&opts, parsed.rest.first().map(String::as_str).unwrap_or_default(), &mut log).await,
            "rebase" => reylo_migrate::rebase(&opts, &parsed.rest, &mut log).await,
            "repair" => reylo_migrate::repair(&opts, &mut log).await,
            other => {
                eprintln!("reylo migrate: unknown command {other}\n\n{MIGRATE_USAGE}");
                Ok(2)
            }
        }
    });
    result.map_err(|e| e.0)
}

fn run_command(args: &[String]) -> Result<i32, String> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{RUN_USAGE}");
        return Ok(0);
    }
    let parsed = parse(args)?;
    let opts = settings(&parsed, false)?;
    let mut items: Vec<(String, String)> = parsed.rest.iter().map(|sql| (if sql.chars().count() > 60 { format!("{}…", sql.chars().take(57).collect::<String>()) } else { sql.clone() }, sql.clone())).collect();
    for f in &parsed.files {
        let sql = std::fs::read_to_string(Path::new(f)).map_err(|e| format!("cannot read {f}: {e}"))?;
        items.push((f.clone(), sql));
    }
    if items.is_empty() {
        println!("{RUN_USAGE}");
        return Ok(2);
    }
    let mut log = print;
    runtime().block_on(reylo_migrate::run_sql(&opts.url, &opts.root, &items, &mut log)).map_err(|e| e.0)
}

fn engine_command(args: &[String]) -> i32 {
    let call = args.get(1).map(String::as_str) == Some("call");
    match args.first().map(String::as_str) {
        Some("postgres") => reylo_engine_native::serve(reylo_engine_native::postgres::Postgres::default(), call),
        Some("mysql") => reylo_engine_native::serve(reylo_engine_native::mysql::Mysql::default(), call),
        Some("sqlite") => reylo_engine_native::serve(reylo_engine_native::sqlite::Sqlite::default(), call),
        Some("duckdb") => reylo_engine_native::serve(reylo_engine_native::duckdb::Duckdb, call),
        other => {
            eprintln!("reylo engine: name a database: postgres, mysql, sqlite or duckdb{}", other.map(|o| format!(" (not {o})")).unwrap_or_default());
            return 2;
        }
    }
    0
}

fn main() {
    reylo_migrate::commands::set_color(std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        None | Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            0
        }
        Some("--version" | "-V") => {
            println!("reylo {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("analyze") => analyze::run(&args[1..]),
        Some("engine") => engine_command(&args[1..]),
        Some("play") => {
            eprintln!("reylo play runs from reylo's npm package, which renders the playground's TypeScript: npx reylo play");
            2
        }
        Some("generate") => {
            eprintln!("reylo generate was removed in 0.8.0: the type check and the build steps fill .reylo/ where the database can be reached, and reylo analyze prints each statement's types.");
            2
        }
        Some("migrate") => migrate_command(&args[1..]).unwrap_or_else(|e| {
            eprintln!("reylo migrate: {e}");
            1
        }),
        Some("run") => run_command(&args[1..]).unwrap_or_else(|e| {
            eprintln!("reylo run: {e}");
            1
        }),
        Some(other) => {
            eprintln!("reylo: unknown command {other}\n\n{USAGE}");
            2
        }
    };
    std::process::exit(code);
}
