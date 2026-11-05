//! `reylo`, one program for every database: the analysis engine a language's frontend talks to over
//! standard input and output (`reylo engine <database>`, one JSON request per line, one answer per
//! line), each statement's type descriptor on the command line (`reylo compile`), and the
//! migrations (`reylo migrate`, `reylo run`). npm's `reylo` command runs it (src/cli.ts), but for
//! `reylo play`, the playground's server, which renders TypeScript and so runs in JavaScript.

mod compile;
mod project;

use project::Project;
use reylo_migrate::Options;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::Path;

const USAGE: &str = "Usage: reylo <command>

  compile <paths> [--url]   Print each statement's type descriptor, one line of JSON per
                            statement, read from the project's database
  migrate <command>         Write, check and apply migrations (reylo migrate --help)
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
--file names (-f), in the order given. The database is the type check's connection; --url names
another.";

const MIGRATE_USAGE: &str = "Usage: reylo migrate <command> [flags]

Commands:
  init                    Start from this database
    --schema <name>       A schema to read (Postgres; public by default)
  create <name>           Write a migration from your change to schema.sql
    --drop <names>        Accept what it drops: tables and columns
    --rename <old>=<new>  Say a table or column was renamed
  dev                     Take this database to the migrations, and lock them
  prod                    Run what the lock lists, with no shadow database
  sync                    Take this database to schema.sql, unrecorded
    --watch               Again on each change
  check                   Check the migrations, schema.sql and this database
    --offline             Check only what needs no database
  reset                   Drop everything in this database
  wipe                    Delete every row, keeping the schema
  pull                    Write the migrations this database ran into files
  sync-history            Record what this database's schema already has
  lock                    Write the lock from what this database ran
  unlock                  Delete the lock: migrations run by name

Flags:
  --url <url>             The database, in place of direct_url
  --dir <path>            Reylo's directory, in place of dir
  -h, --help              Print this help";

/// The help as a terminal shows it: its headings bold, each command cyan, each flag yellow and what
/// it takes dim. Piped, it is the text alone.
fn painted_usage() -> String {
    use reylo_migrate::commands::paint;
    let name = regex::Regex::new(r"^(\s+)(--?[a-z-]+(?:, --[a-z-]+)?|[a-z][a-z-]*)( <\S+)?(\s{2}.*)$").unwrap();
    let lines = MIGRATE_USAGE.lines().map(|line| {
        if let Some(rest) = line.strip_prefix("Usage: ") {
            return format!("{} {rest}", paint("bold", "Usage:"));
        }
        if !line.starts_with(' ') && line.ends_with(':') {
            return paint("bold", line);
        }
        let Some(c) = name.captures(line) else { return line.to_string() };
        let style = if c[2].starts_with('-') { "yellow" } else { "cyan" };
        let takes = c.get(3).map_or(String::new(), |t| paint("dim", t.as_str()));
        format!("{}{}{takes}{}", &c[1], paint(style, &c[2]), &c[4])
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// The arguments after a command: flags with values, lists, switches and the rest.
#[derive(Default)]
struct Args {
    values: HashMap<String, String>,
    schemas: Vec<String>,
    files: Vec<String>,
    drops: Vec<String>,
    renames: Vec<(String, String)>,
    switches: Vec<String>,
    rest: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut k = 0;
    while k < args.len() {
        let a = &args[k];
        // A flag's value may follow an `=`: --url=<url>, --rename=<old>=<new>.
        let (a, inline) = match a.split_once('=') {
            Some((flag, v)) if flag.starts_with("--") => (flag, Some(v.to_string())),
            _ => (a.as_str(), None),
        };
        let mut value = || -> Result<String, String> {
            if let Some(v) = &inline {
                return Ok(v.clone());
            }
            k += 1;
            args.get(k).filter(|v| !v.starts_with("--")).cloned().ok_or_else(|| format!("{a} needs a value"))
        };
        match a {
            "--url" | "--dir" => {
                let v = value()?;
                out.values.insert(a[2..].to_string(), v);
            }
            "--schema" => out.schemas.push(value()?),
            "--file" | "-f" => out.files.push(value()?),
            "--drop" => out.drops.extend(value()?.split(',').map(str::trim).filter(|n| !n.is_empty()).map(String::from)),
            "--rename" => {
                let v = value()?;
                let (old, new) = v.split_once('=').ok_or_else(|| format!("--rename takes <table>.<column>=<new name>, not {v}"))?;
                out.renames.push((old.trim().to_string(), new.trim().to_string()));
            }
            s if s.starts_with("--") => out.switches.push(args[k][2..].to_string()),
            _ => out.rest.push(args[k].clone()),
        }
        k += 1;
    }
    Ok(out)
}

/// The project's `migrate` settings, flags over the file. `require` refuses a project with no
/// `migrate` section, whose presence opts the project in.
fn settings(args: &Args, require: bool) -> Result<Options, String> {
    settings_with(args, require, true)
}

/// As `settings`, where `need_url` says whether a command can't run without a database.
fn settings_with(args: &Args, require: bool, need_url: bool) -> Result<Options, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let project = Project::resolve(&cwd, std::env::vars().collect())?;
    let m = project.config.migrate.clone();
    if m.is_none() && require && !args.values.contains_key("url") {
        let file = project.file.as_deref().map(|f| f.file_name().unwrap_or_default().to_string_lossy().into_owned()).unwrap_or_else(|| ".reylo.yaml".into());
        return Err(format!("no migrate section in {file}: start with reylo migrate init"));
    }
    let m = m.unwrap_or_default();
    let url = match args.values.get("url") {
        Some(url) => url.clone(),
        None => project.type_check_url().unwrap_or_default().unwrap_or_default(),
    };
    if url.is_empty() && need_url {
        return Err("no database: set direct_url in .reylo.yaml, or DATABASE_URL".into());
    }
    if !url.is_empty() {
        reylo_migrate::Dialect::of_url(&url).map_err(|e| e.0)?;
    }
    let shadow_url = m.shadow_url.as_deref().map(|u| project.expand("migrate.shadow_url", u)).transpose()?;
    // The project's own files, in the directory `dir` names (`reylo`), or --dir for a run.
    let home = match args.values.get("dir") {
        Some(v) => cwd.join(v),
        None => project.root.join(project.config.dir.as_deref().unwrap_or("reylo")),
    };
    Ok(Options {
        url,
        shadow_url,
        dir: home.join("migrations"),
        schema: home.join("schema.sql"),
        locked: m.lock != Some(false),
        lock: home.join(reylo_migrate::lock::LOCK),
        cache: home.join(".cache"),
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
        println!("{}", painted_usage());
        return Ok(0);
    };
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) || command == "help" {
        println!("{}", painted_usage());
        return Ok(0);
    }
    if !["create", "dev", "sync", "reset", "wipe", "check", "prod", "pull", "sync-history", "init", "lock", "unlock"].contains(&command.as_str()) {
        eprintln!("reylo migrate: unknown command {command}\n\n{MIGRATE_USAGE}");
        return Ok(2);
    }
    let parsed = parse(&args[1..])?;
    let has = |s: &str| parsed.switches.iter().any(|x| x == s);
    // What a command doesn't take is refused, never passed over: prod --dev must not run prod, and
    // sync --rename must not leave its guess unsaid.
    let takes: &[&str] = match command.as_str() {
        "sync" => &["watch"],
        "check" => &["offline"],
        _ => &[],
    };
    let refused = |flag: &str| Err(format!("{command} doesn't take {flag} (reylo migrate --help)"));
    if let Some(flag) = parsed.switches.iter().find(|s| !takes.contains(&s.as_str())) {
        return refused(&format!("--{flag}"));
    }
    if command != "create" && !parsed.drops.is_empty() {
        return refused("--drop");
    }
    if command != "create" && !parsed.renames.is_empty() {
        return refused("--rename");
    }
    if command != "init" && !parsed.schemas.is_empty() {
        return refused("--schema");
    }
    if !parsed.files.is_empty() {
        return refused("--file");
    }
    if let Some(word) = parsed.rest.get(if command == "create" { 1 } else { 0 }) {
        return refused(word);
    }
    let offline = command == "check" && has("offline");
    // init is how a project starts: it adds the section the rest need.
    let opts = settings_with(&parsed, command != "init", !offline && command != "unlock")?;
    if offline {
        return reylo_migrate::check_offline(&opts, &mut print).map_err(|e| e.0);
    }
    if command == "unlock" {
        return unlock(&opts);
    }
    // The development commands change a database in ways no deploy wants.
    let in_ci = std::env::var_os("CI").is_some();
    let mut log = print;
    let rt = runtime();
    let result = rt.block_on(async {
        match command.as_str() {
            "prod" => reylo_migrate::prod(&opts, &mut log).await,
            "dev" => reylo_migrate::dev(&opts, in_ci, &mut log).await,
            "reset" => reylo_migrate::reset(&opts, in_ci, &mut log).await,
            "wipe" => reylo_migrate::wipe_rows(&opts, in_ci, &mut log).await,
            "sync" if has("watch") => reylo_migrate::sync_watch(&opts, in_ci, None, &mut log).await,
            "sync" => reylo_migrate::sync(&opts, in_ci, &mut log).await,
            "check" => reylo_migrate::check_command(&opts, &mut log).await,
            "sync-history" => reylo_migrate::sync_history(&opts, &mut log).await,
            "lock" => match reylo_migrate::lock_command(&opts, &mut log).await {
                Ok(0) => set_lock(&opts.root, true).map(|()| 0).map_err(reylo_migrate::Error),
                other => other,
            },
            "pull" => reylo_migrate::pull(&opts, &mut log).await,
            // The section follows the files, whatever came of the rest: a run that wrote them and
            // failed is finished by the commands that need it, and init refuses to write them twice.
            "init" => {
                let result = reylo_migrate::init(&opts, &parsed.schemas, &mut log).await;
                if opts.schema.exists() {
                    add_migrate_section(&opts.root).map_err(reylo_migrate::Error)?;
                }
                result
            }
            "create" => {
                let choices = reylo_migrate::Choices { drop: parsed.drops.clone(), rename: parsed.renames.clone() };
                reylo_migrate::create(&opts, parsed.rest.first().map(String::as_str).unwrap_or_default(), &choices, &mut log).await
            }
            other => Err(reylo_migrate::Error(format!("unknown command {other}"))),
        }
    });
    result.map_err(|e| e.0)
}

/// `reylo migrate unlock`: deletes the lock and writes `lock: false` into .reylo.yaml, so the
/// migrations run by name, and one older than one applied runs where both orders end the same.
fn unlock(opts: &Options) -> Result<i32, String> {
    set_lock(&opts.root, false)?;
    let had = opts.lock.exists();
    if had {
        std::fs::remove_file(&opts.lock).map_err(|e| format!("cannot delete {}: {e}", opts.lock.display()))?;
    }
    let name = opts.lock.file_name().unwrap_or_default().to_string_lossy();
    println!("✓ Unlocked: {}.reylo.yaml says lock: false", if had { format!("deleted {name}; ") } else { String::new() });
    println!("  reylo migrate lock locks it again, in the order a database applied the migrations");
    Ok(0)
}

/// The project's file: the one there is, else where a new one goes.
fn project_file(root: &Path) -> std::path::PathBuf {
    [".reylo.yaml", ".reylo.yml"].iter().map(|n| root.join(n)).find(|p| p.exists()).unwrap_or_else(|| root.join(".reylo.yaml"))
}

/// Sets `lock` in the migrate section of the project's .reylo.yaml. Edits the one line, keeping the
/// rest as written.
fn set_lock(root: &Path, on: bool) -> Result<(), String> {
    let file = project_file(root);
    let text = std::fs::read_to_string(&file).map_err(|_| "no .reylo.yaml in the project: start with reylo migrate init".to_string())?;
    let by_hand = || format!("couldn't edit the migrate section of {}: set migrate.lock to {on} by hand", file.display());
    let edited = with_lock(&text, on).ok_or_else(by_hand)?;
    // The edit stands only where the file then reads as asked.
    let read = reylo_engine_core::config::parse(&edited).ok().and_then(|p| p.migrate).and_then(|m| m.lock);
    if read != Some(on) {
        return Err(by_hand());
    }
    if edited != text {
        std::fs::write(&file, edited).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Adds the migrate section to the project's .reylo.yaml, making the file where there is none; a
/// section already there is left as it is.
fn add_migrate_section(root: &Path) -> Result<(), String> {
    let file = project_file(root);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if reylo_engine_core::config::parse(&text).map_err(|e| e.message)?.migrate.is_some() {
        return Ok(());
    }
    let gap = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
    std::fs::write(&file, format!("{text}{gap}migrate:\n  lock: true\n")).map_err(|e| e.to_string())?;
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    println!("{} Added migrate to {name}", reylo_migrate::commands::paint("green", "✓"));
    Ok(())
}

/// The text of a .reylo.yaml with its migrate section's `lock` line set; None where the section
/// isn't one a line edit can change (missing, or written inline with keys).
fn with_lock(text: &str, on: bool) -> Option<String> {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header = regex::Regex::new(r"^migrate:\s*(\{\s*\})?\s*(#.*)?$").unwrap();
    let at = lines.iter().position(|l| header.is_match(l))?;
    if lines[at].contains('{') {
        lines[at] = match lines[at].find('#') {
            Some(c) => format!("migrate: {}", &lines[at][c..]),
            None => "migrate:".into(),
        };
    }
    // The section's lines: indented ones, and blank lines and comments between them.
    let inside = |l: &str| l.trim().is_empty() || l.starts_with([' ', '\t']) || l.trim_start().starts_with('#');
    let end = (at + 1..lines.len()).find(|&k| !inside(&lines[k])).unwrap_or(lines.len());
    let key = regex::Regex::new(r"^(\s+)lock:").unwrap();
    let found = (at + 1..end).find(|&k| key.is_match(&lines[k]));
    let indent = (at + 1..end).find(|&k| lines[k].starts_with([' ', '\t']) && !lines[k].trim().is_empty() && !lines[k].trim_start().starts_with('#')).map(|k| lines[k][..lines[k].len() - lines[k].trim_start().len()].to_string()).unwrap_or_else(|| "  ".into());
    match found {
        Some(k) => lines[k] = format!("{indent}lock: {on}"),
        None => lines.insert(at + 1, format!("{indent}lock: {on}")),
    }
    let mut out = lines.join(newline);
    if text.ends_with('\n') {
        out.push_str(newline);
    }
    Some(out)
}

fn run_command(args: &[String]) -> Result<i32, String> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{RUN_USAGE}");
        return Ok(0);
    }
    let parsed = parse(args)?;
    if let Some(flag) = parsed.switches.first() {
        return Err(format!("run doesn't take --{flag}"));
    }
    if !parsed.drops.is_empty() || !parsed.renames.is_empty() || !parsed.schemas.is_empty() {
        return Err("run takes SQL, --file and --url".into());
    }
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
        Some("compile") => compile::run(&args[1..]),
        Some("engine") => engine_command(&args[1..]),
        Some("play") => {
            eprintln!("reylo play runs from reylo's npm package, which renders the playground's TypeScript: npx reylo play");
            2
        }
        Some("generate") => {
            eprintln!("reylo generate was removed in 0.8.0: the type check and the build steps fill reylo/.cache/ where the database can be reached, and reylo compile prints each statement's types.");
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

#[cfg(test)]
mod tests {
    use super::{painted_usage, with_lock, MIGRATE_USAGE};

    #[test]
    fn the_help_is_its_text_when_nothing_paints_it() {
        assert_eq!(painted_usage(), MIGRATE_USAGE);
    }

    #[test]
    fn lock_is_set_in_the_migrate_section() {
        let file = "# the project\nmigrate:\n  shadow_url: ${SHADOW_URL}   # the CI server's\n\nqueries: src\n";
        let off = with_lock(file, false).unwrap();
        assert_eq!(off, "# the project\nmigrate:\n  lock: false\n  shadow_url: ${SHADOW_URL}   # the CI server's\n\nqueries: src\n");
        assert_eq!(with_lock(&off, false).unwrap(), off);
        assert_eq!(with_lock(&off, true).unwrap(), off.replace("lock: false", "lock: true"));
        assert_eq!(with_lock("migrate: {}\n", false).unwrap(), "migrate:\n  lock: false\n");
        assert_eq!(with_lock("migrate:\n    lock: true\n", false).unwrap(), "migrate:\n    lock: false\n");
        assert_eq!(with_lock("queries: src\n", true), None);
        // A comment at the start of a line inside the section is the section's.
        assert_eq!(with_lock("migrate:\n# the shadow server\n  shadow_url: x\n  lock: false\n", true).unwrap(), "migrate:\n# the shadow server\n  shadow_url: x\n  lock: true\n");
        assert_eq!(with_lock("migrate: {} # opt in\n", false).unwrap(), "migrate: # opt in\n  lock: false\n");
    }
}
