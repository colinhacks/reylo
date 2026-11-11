//! `reylo`, one program for every database: the analysis engine a language's frontend talks to over
//! standard input and output (`reylo engine <database>`, one JSON request per line, one answer per
//! line), each statement's type descriptor on the command line (`reylo analyze`), and the
//! migrations (`reylo migrate`, `reylo db`, `reylo run`). npm's `reylo` command runs it (src/cli.ts), but for
//! `reylo play`, the playground's server, which renders TypeScript and so runs in JavaScript.

mod analyze;
mod project;

use project::Project;
use reylo_migrate::Options;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::Path;

const USAGE: &str = "Usage: reylo <command>

  analyze <paths> [--url]   Print each statement's type descriptor, one line of JSON per
                            statement, read from the project's database
  migrate <command>         Write, check and run migrations, and repair their history
  db <command>              Push schema.sql to a development database, reset, wipe or read it
                            (reylo migrate --help lists both groups' commands)
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

const MIGRATE_USAGE: &str = "Usage: reylo <group> <command> [flags]

Commands:
  migrate init             Start from this database
    --schema <name>        A schema to read (Postgres; public by default)
  migrate new <name>       Write the next migration, from schema.sql or this database
    --drop <names>         Accept what it drops: tables and columns
    --rename <old>=<new>   Say a table or column was renamed
  migrate dev              Take this database to the migrations, and lock them
  migrate deploy           Run what the lock lists, with no shadow database
    --dry-run              Try it on a shadow database, running nothing here
  migrate status           Show what this database ran, what's pending and what changed
  migrate lint             Check the migrations, the lock and schema.sql, with no database
  migrate lock             Write the lock from this database's history
  migrate history push     Record the migrations this database's schema has
  migrate history pull     Write the migrations this database ran into files
  db push                  Take this database to schema.sql, unrecorded
    --watch                Push again on each save
  db reset                 Rebuild this database from the migrations
  db wipe                  Delete every row, keeping the schema
  db introspect            Print this database's schema as SQL
    --schema <name>        A schema to read (Postgres; public by default)

Flags:
  --url <url>              The database, in place of direct_url
  --json                   Print the result as one JSON object
  -h, --help               Print this help";

/// Every command of those groups, as it's typed.
const COMMANDS: [&str; 13] = ["migrate init", "migrate new", "migrate dev", "migrate deploy", "migrate status", "migrate lint", "migrate lock", "migrate history push", "migrate history pull", "db push", "db reset", "db wipe", "db introspect"];

/// Names another tool gives these commands, and the one each is here.
const ALSO_KNOWN_AS: [(&str, &str); 3] = [("create", "migrate new"), ("generate", "migrate new"), ("pull", "db introspect")];

/// The help as a terminal shows it: its headings bold, each command cyan, each flag yellow and what
/// it takes dim. Piped, it is the text alone.
fn painted_usage() -> String {
    use reylo_migrate::commands::paint;
    let name = regex::Regex::new(r"^(\s+)(--?[a-z-]+(?:, --[a-z-]+)?|[a-z][a-z -]*?)( <\S+)?(\s{2}.*)$").unwrap();
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
            "--url" => {
                let v = value()?;
                out.values.insert(a[2..].to_string(), v);
            }
            "--schema" => out.schemas.push(value()?),
            "--file" | "-f" => out.files.push(value()?),
            "--drop" => out.drops.extend(value()?.split(',').map(str::trim).filter(|n| !n.is_empty()).map(String::from)),
            "--rename" => {
                let v = value()?;
                let (old, new) = v.split_once('=').ok_or_else(|| format!("--rename takes <old name>=<new name>, a table as users=accounts or a column as users.bio=about, not {v}"))?;
                out.renames.push((old.trim().to_string(), new.trim().to_string()));
            }
            s if s.starts_with("--") => out.switches.push(args[k][2..].to_string()),
            _ => out.rest.push(args[k].clone()),
        }
        k += 1;
    }
    Ok(out)
}

/// The project's settings for these commands, `--url` over the file. `need_url` says whether the
/// command can't run without a database.
fn settings(args: &Args, need_url: bool) -> Result<Options, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let project = Project::resolve(&cwd, environment())?;
    let url = match args.values.get("url") {
        Some(url) => url.clone(),
        None => project.type_check_url().unwrap_or_default().map(|(url, _)| url).unwrap_or_default(),
    };
    if url.is_empty() && need_url {
        return Err("no database: set direct_url in .reylo.yaml, or DATABASE_URL".into());
    }
    if !url.is_empty() {
        reylo_migrate::Dialect::of_url(&url).map_err(|e| e.0)?;
    }
    let shadow_url = project.config.shadow_url.as_deref().map(|u| project.expand("shadow_url", u)).transpose()?;
    // The project's own files, in the directory `dir` names (`reylo`).
    let home = project.root.join(project.config.dir.as_deref().unwrap_or("reylo"));
    Ok(Options {
        url,
        shadow_url,
        dir: home.join("migrations"),
        schema: home.join("schema.sql"),
        lock: home.join(reylo_migrate::lock::LOCK),
        cache: home.join(".cache"),
        root: project.root.clone(),
    })
}

/// The process environment, a variable that isn't UTF-8 read lossily, as `reylo analyze` reads it.
fn environment() -> std::collections::HashMap<String, String> {
    std::env::vars_os().map(|(name, value)| (name.to_string_lossy().into_owned(), value.to_string_lossy().into_owned())).collect()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime")
}

fn print(line: &str) {
    println!("{line}");
}

/// A command that isn't one, said on stderr with exit 2; with `--json`, as the one JSON object every
/// command prints, `extra` (the usage) left out.
fn usage_error(group: &str, args: &[String], message: &str, extra: &str) -> i32 {
    if args.iter().any(|a| a == "--json") {
        let named: Vec<&str> = args.iter().take_while(|a| !a.starts_with('-')).map(String::as_str).collect();
        let out = serde_json::json!({ "command": format!("reylo {group} {}", named.join(" ")).trim_end(), "ok": false, "exit": 2, "error": message, "log": [] });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        eprintln!("{message}{extra}");
    }
    2
}

/// The commands of `reylo migrate` and `reylo db`; with `--json`, a command refused before it starts
/// is one JSON object too.
fn group_command(group: &str, args: &[String]) -> Result<i32, String> {
    let result = group_command_checked(group, args);
    match result {
        Err(e) if args.iter().any(|a| a == "--json") => {
            let named: Vec<&str> = args.iter().take_while(|a| !a.starts_with('-')).map(String::as_str).collect();
            let out = serde_json::json!({ "command": format!("reylo {group} {}", named.join(" ")).trim_end(), "ok": false, "exit": 1, "error": e, "log": [] });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            Ok(1)
        }
        other => other,
    }
}

fn group_command_checked(group: &str, args: &[String]) -> Result<i32, String> {
    if args.is_empty() || args[0] == "help" || args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        println!("{}", painted_usage());
        return Ok(0);
    }
    // The words that name the command: `migrate history` takes one more.
    let words = if group == "migrate" && args[0] == "history" { args.len().min(2) } else { 1 };
    let command = format!("{group} {}", args[..words].join(" "));
    if group == "migrate" && args[0] == "history" && !COMMANDS.contains(&command.as_str()) {
        let message = match args.get(1).filter(|a| !a.starts_with('-')) {
            Some(word) => format!("reylo migrate history has no {word}: it takes push or pull"),
            None => "reylo migrate history takes push or pull".to_string(),
        };
        return Ok(usage_error(group, args, &message, ""));
    }
    if !COMMANDS.contains(&command.as_str()) {
        // A command of another group, whose names are near each other's, is named.
        let word = args[words - 1].as_str();
        let mut theirs: Vec<String> = COMMANDS.iter().filter(|c| c.rsplit(' ').next() == Some(word) && **c != command).map(|c| format!("reylo {c}")).collect();
        if theirs.is_empty() {
            theirs = ALSO_KNOWN_AS.iter().filter(|(name, _)| *name == word).map(|(_, c)| format!("reylo {c}")).collect();
        }
        if !theirs.is_empty() {
            return Ok(usage_error(group, args, &format!("reylo {group} has no {word}: it's {}", theirs.join(", or ")), ""));
        }
        return Ok(usage_error(group, args, &format!("reylo {group} has no command {word} (reylo {group} --help lists them)"), ""));
    }
    let command = command.as_str();
    let parsed = parse(&args[words..])?;
    let has = |s: &str| parsed.switches.iter().any(|x| x == s);
    // What a command doesn't take is refused, never passed over: deploy --dev must not run deploy, and
    // db push --rename must not leave its guess unsaid.
    let takes: &[&str] = match command {
        "db push" => &["watch"],
        "migrate deploy" => &["dry-run"],
        _ => &[],
    };
    let short = args[..words].join(" ");
    let refused = |flag: &str| Ok(usage_error(group, args, &format!("reylo {group} {short} doesn't take {flag} (reylo {group} --help)"), ""));
    // Every command takes --json, but for one that runs until it's stopped.
    if let Some(flag) = parsed.switches.iter().find(|s| *s != "json" && !takes.contains(&s.as_str())) {
        return refused(&format!("--{flag}"));
    }
    let json = has("json");
    if json && has("watch") {
        return Ok(usage_error(group, args, "reylo db push --watch runs until it's stopped, so it can't print one JSON object", ""));
    }
    if command != "migrate new" && !parsed.drops.is_empty() {
        return refused("--drop");
    }
    if command != "migrate new" && !parsed.renames.is_empty() {
        return refused("--rename");
    }
    if !matches!(command, "migrate init" | "db introspect") && !parsed.schemas.is_empty() {
        return refused("--schema");
    }
    if !parsed.files.is_empty() {
        return refused("--file");
    }
    if let Some(word) = parsed.rest.get(if command == "migrate new" { 1 } else { 0 }) {
        return refused(word);
    }
    if !json {
        let mut sql = None;
        let code = run_group(command, &parsed, &mut print, &mut sql);
        // The schema alone on stdout, so that it can be saved with a redirect.
        if let Some(sql) = sql {
            print!("{sql}");
        }
        return code;
    }
    // With --json, one JSON object on stdout: whether it succeeded, each refusal with its code, what
    // the command found (status's rows), and every line it would have printed, plain.
    reylo_migrate::commands::set_color(false);
    reylo_migrate::commands::set_json(true);
    let mut lines: Vec<String> = Vec::new();
    let mut sql = None;
    let outcome = run_group(command, &parsed, &mut |l: &str| lines.push(l.to_string()), &mut sql);
    let named = if command == "migrate deploy" && has("dry-run") { "migrate deploy --dry-run".to_string() } else { command.to_string() };
    let mut out = serde_json::json!({ "command": format!("reylo {named}") });
    let (code, error) = match outcome {
        Ok(code) => (code, None),
        Err(e) => (1, Some(e)),
    };
    out["ok"] = (code == 0).into();
    out["exit"] = code.into();
    if let Some(e) = error {
        out["error"] = e.into();
    }
    let problems = reylo_migrate::commands::reported();
    if !problems.is_empty() {
        out["problems"] = problems.into();
    }
    if let Some(result) = reylo_migrate::commands::result() {
        out["result"] = result;
    }
    if let Some(sql) = sql {
        out["sql"] = sql.into();
    }
    out["log"] = lines.into();
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    Ok(code)
}

/// Runs one command of `reylo migrate` or `reylo db`, its flags checked; `db introspect`'s SQL goes to
/// `sql`, for the caller to print.
fn run_group(command: &str, parsed: &Args, log: &mut dyn FnMut(&str), sql: &mut Option<String>) -> Result<i32, String> {
    let has = |s: &str| parsed.switches.iter().any(|x| x == s);
    let opts = settings(parsed, command != "migrate lint")?;
    // The home directory is no project's (project.rs), so nothing makes one there.
    if matches!(command, "migrate init" | "migrate history pull") && project::is_home(&opts.root, &environment()) {
        return Err(format!("{} won't write into your home directory: run it in your project's directory", command.trim_start_matches("migrate ")));
    }
    // These start from the database; the rest need a project to work on.
    if !matches!(command, "migrate init" | "migrate history pull" | "db introspect" | "db wipe") && !opts.schema.exists() && !opts.dir.exists() {
        let home = opts.schema.parent().unwrap_or(&opts.root);
        return Err(format!("no {}/ directory here: start one with reylo migrate init, or set dir in .reylo.yaml", home.strip_prefix(&opts.root).unwrap_or(home).display()));
    }
    if command == "migrate lint" {
        return reylo_migrate::lint(&opts, log).map_err(|e| e.0);
    }
    // The development commands change a database in ways no deploy wants.
    let in_ci = std::env::var_os("CI").is_some();
    let mut log = log;
    let rt = runtime();
    let result = rt.block_on(async {
        match command {
            "migrate init" => reylo_migrate::init(&opts, &parsed.schemas, &mut log).await,
            "migrate new" => {
                let choices = reylo_migrate::Choices { drop: parsed.drops.clone(), rename: parsed.renames.clone() };
                reylo_migrate::create(&opts, parsed.rest.first().map(String::as_str).unwrap_or_default(), &choices, &mut log).await
            }
            "migrate history push" => reylo_migrate::history_push(&opts, &mut log).await,
            "migrate history pull" => reylo_migrate::history_pull(&opts, &mut log).await,
            "migrate status" => reylo_migrate::status(&opts, &mut log).await,
            "migrate lock" => reylo_migrate::lock_from_history(&opts, &mut log).await,
            "migrate dev" => reylo_migrate::dev(&opts, in_ci, &mut log).await,
            "migrate deploy" if has("dry-run") => reylo_migrate::dry_run(&opts, &mut log).await,
            "migrate deploy" => reylo_migrate::deploy(&opts, &mut log).await,
            "db push" if has("watch") => reylo_migrate::db_push_watch(&opts, in_ci, None, &mut log).await,
            "db push" => reylo_migrate::db_push(&opts, in_ci, &mut log).await,
            "db introspect" => reylo_migrate::db_introspect(&opts, &parsed.schemas).await.map(|text| {
                *sql = Some(text);
                0
            }),
            "db reset" => reylo_migrate::db_reset(&opts, in_ci, &mut log).await,
            "db wipe" => reylo_migrate::db_wipe(&opts, in_ci, &mut log).await,
            other => Err(reylo_migrate::Error(format!("unknown command {other}"))),
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
    if let Some(flag) = parsed.switches.first() {
        eprintln!("reylo run doesn't take --{flag} (reylo run --help)");
        return Ok(2);
    }
    if !parsed.drops.is_empty() || !parsed.renames.is_empty() || !parsed.schemas.is_empty() {
        return Err("run takes SQL, --file and --url".into());
    }
    let opts = settings(&parsed, true)?;
    let mut items: Vec<(String, String)> = parsed.rest.iter().map(|sql| (if sql.chars().count() > 60 { format!("{}…", sql.chars().take(57).collect::<String>()) } else { sql.clone() }, sql.clone())).collect();
    for f in &parsed.files {
        let sql = std::fs::read_to_string(Path::new(f)).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { format!("no file {f}") } else { format!("can't read {f}: {}", e.kind()) })?;
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
            eprintln!("reylo generate was removed in 0.8.0: the type check and the build steps fill reylo/.cache/ where the database can be reached, and reylo analyze prints each statement's types.");
            2
        }
        Some("migration") => {
            // The group these commands had before they were `reylo migrate`'s: the command named, where
            // it is one, else every command ending in its last word.
            let take = if args.get(1).map(String::as_str) == Some("history") { 2 } else { 1 };
            let words: Vec<&str> = args[1..].iter().take_while(|w| !w.starts_with('-')).take(take).map(|w| if w == "create" { "new" } else { w.as_str() }).collect();
            let named = format!("migrate {}", words.join(" "));
            let found: Vec<String> = if COMMANDS.contains(&named.as_str()) {
                vec![format!("reylo {named}")]
            } else {
                COMMANDS.iter().filter(|c| words.last().is_some_and(|w| c.ends_with(&format!(" {w}")))).map(|c| format!("reylo {c}")).collect()
            };
            let message = if found.is_empty() { "reylo migration is reylo migrate (reylo migrate --help)".to_string() } else { format!("reylo migration is reylo migrate: {}", found.join(", or ")) };
            usage_error("migration", &args[1..], &message, "")
        }
        Some(group @ ("migrate" | "db")) => group_command(group, &args[1..]).unwrap_or_else(|e| {
            eprintln!("reylo {group}: {e}");
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
    use super::{painted_usage, COMMANDS, MIGRATE_USAGE};

    #[test]
    fn the_help_is_its_text_when_nothing_paints_it() {
        assert_eq!(painted_usage(), MIGRATE_USAGE);
    }

    #[test]
    fn the_help_lists_every_command() {
        for command in COMMANDS {
            assert!(MIGRATE_USAGE.contains(&format!("  {command}  ")) || MIGRATE_USAGE.contains(&format!("  {command} <")), "{command}");
        }
    }
}
