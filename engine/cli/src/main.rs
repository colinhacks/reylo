//! `reylo`, one program for every database: the analysis engine a language's frontend talks to over
//! standard input and output (`reylo engine <database>`, one JSON request per line, one answer per
//! line), each statement's type descriptor on the command line (`reylo analyze`), and the
//! migrations (`reylo migration`, `reylo migrate`, `reylo db`, `reylo run`). npm's `reylo` command runs it (src/cli.ts), but for
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
  migration <command>       Write and lint migrations, and repair their history
  migrate <command>         Run migrations on a database
  db <command>              Change a database directly, recording nothing
                            (reylo migrate --help lists all three's commands)
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
  migration init           Start from this database
    --schema <name>        A schema to read (Postgres; public by default)
  migration create <name>  Write a migration from your change to schema.sql
    --drop <names>         Accept what it drops: tables and columns
    --rename <old>=<new>   Say a table or column was renamed
  migration lint           Check the migrations and schema.sql, with no database
  migration history push   Record the migrations this database's schema has
  migration history pull   Write the migrations this database ran into files
  migrate dev              Take this database to the migrations, and lock them
  migrate deploy           Run what the lock lists, with no shadow database
    --dry-run              Try it on a shadow database, running nothing here
  db push                  Take this database to schema.sql, unrecorded
    --watch                Again on each change
  db introspect            Print this database's schema as SQL
    --schema <name>        A schema to read (Postgres; public by default)
  db wipe                  Delete every row, keeping the schema

Flags:
  --url <url>              The database, in place of direct_url
  -h, --help               Print this help";

/// Every command of those groups, as it's typed.
const COMMANDS: [&str; 10] = ["migration init", "migration create", "migration lint", "migration history push", "migration history pull", "migrate dev", "migrate deploy", "db push", "db introspect", "db wipe"];

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

/// The project's settings for these commands, `--url` over the file. `need_url` says whether the
/// command can't run without a database.
fn settings(args: &Args, need_url: bool) -> Result<Options, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let project = Project::resolve(&cwd, std::env::vars().collect())?;
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

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime")
}

fn print(line: &str) {
    println!("{line}");
}

/// The commands of `reylo migration`, `reylo migrate` and `reylo db`.
fn group_command(group: &str, args: &[String]) -> Result<i32, String> {
    if args.is_empty() || args[0] == "help" || args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        println!("{}", painted_usage());
        return Ok(0);
    }
    // The words that name the command: `migration history` takes one more.
    let words = if group == "migration" && args[0] == "history" { args.len().min(2) } else { 1 };
    let command = format!("{group} {}", args[..words].join(" "));
    if !COMMANDS.contains(&command.as_str()) {
        // A command of another group, whose names are near each other's, is named.
        let word = args[words - 1].as_str();
        let theirs: Vec<String> = COMMANDS.iter().filter(|c| c.rsplit(' ').next() == Some(word)).map(|c| format!("reylo {c}")).collect();
        if !theirs.is_empty() {
            eprintln!("reylo {group} has no {word}: it's {}", theirs.join(", or "));
            return Ok(2);
        }
        eprintln!("reylo {group}: unknown command {word}\n\n{MIGRATE_USAGE}");
        return Ok(2);
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
    let refused = |flag: &str| Err(format!("{short} doesn't take {flag} (reylo {group} --help)"));
    if let Some(flag) = parsed.switches.iter().find(|s| !takes.contains(&s.as_str())) {
        return refused(&format!("--{flag}"));
    }
    if command != "migration create" && !parsed.drops.is_empty() {
        return refused("--drop");
    }
    if command != "migration create" && !parsed.renames.is_empty() {
        return refused("--rename");
    }
    if !matches!(command, "migration init" | "db introspect") && !parsed.schemas.is_empty() {
        return refused("--schema");
    }
    if !parsed.files.is_empty() {
        return refused("--file");
    }
    if let Some(word) = parsed.rest.get(if command == "migration create" { 1 } else { 0 }) {
        return refused(word);
    }
    let opts = settings(&parsed, command != "migration lint")?;
    // These start from the database; the rest need a project to work on.
    if !matches!(command, "migration init" | "migration history pull" | "db introspect" | "db wipe") && !opts.schema.exists() && !opts.dir.exists() {
        let home = opts.schema.parent().unwrap_or(&opts.root);
        return Err(format!("no {} here: check dir in .reylo.yaml, or start with reylo migration init", home.strip_prefix(&opts.root).unwrap_or(home).display()));
    }
    if command == "migration lint" {
        return reylo_migrate::lint(&opts, &mut print).map_err(|e| e.0);
    }
    // The development commands change a database in ways no deploy wants.
    let in_ci = std::env::var_os("CI").is_some();
    let mut log = print;
    let rt = runtime();
    let result = rt.block_on(async {
        match command {
            "migration init" => reylo_migrate::init(&opts, &parsed.schemas, &mut log).await,
            "migration create" => {
                let choices = reylo_migrate::Choices { drop: parsed.drops.clone(), rename: parsed.renames.clone() };
                reylo_migrate::create(&opts, parsed.rest.first().map(String::as_str).unwrap_or_default(), &choices, &mut log).await
            }
            "migration history push" => reylo_migrate::history_push(&opts, &mut log).await,
            "migration history pull" => reylo_migrate::history_pull(&opts, &mut log).await,
            "migrate dev" => reylo_migrate::dev(&opts, in_ci, &mut log).await,
            "migrate deploy" if has("dry-run") => reylo_migrate::dry_run(&opts, &mut log).await,
            "migrate deploy" => reylo_migrate::deploy(&opts, &mut log).await,
            "db push" if has("watch") => reylo_migrate::db_push_watch(&opts, in_ci, None, &mut log).await,
            "db push" => reylo_migrate::db_push(&opts, in_ci, &mut log).await,
            // The schema alone on stdout, so that it can be saved with a redirect.
            "db introspect" => reylo_migrate::db_introspect(&opts, &parsed.schemas).await.map(|sql| {
                print!("{sql}");
                0
            }),
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
        return Err(format!("run doesn't take --{flag}"));
    }
    if !parsed.drops.is_empty() || !parsed.renames.is_empty() || !parsed.schemas.is_empty() {
        return Err("run takes SQL, --file and --url".into());
    }
    let opts = settings(&parsed, true)?;
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
            eprintln!("reylo generate was removed in 0.8.0: the type check and the build steps fill reylo/.cache/ where the database can be reached, and reylo analyze prints each statement's types.");
            2
        }
        Some(group @ ("migration" | "migrate" | "db")) => group_command(group, &args[1..]).unwrap_or_else(|e| {
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
