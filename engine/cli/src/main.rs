//! `reylo`, one program for every database: the analysis engine a language's frontend talks to over
//! standard input and output (`reylo engine <database>`, one JSON request per line, one answer per
//! line), and each statement's type descriptor on the command line (`reylo analyze`). npm's `reylo`
//! command runs it (src/cli.ts), but for `reylo play`, the playground's server, which renders
//! TypeScript and so runs in JavaScript.

mod analyze;
mod project;

const USAGE: &str = "Usage: reylo <command>

  analyze <paths> [--url]   Print each statement's type descriptor, one line of JSON per
                            statement, read from the project's database
  play [--port] [--no-open] Serve the playground on 127.0.0.1, against the project's database
                            (from reylo's npm package: npx reylo play)
  engine <database> [call]  Serve the analysis engine of postgres, mysql, sqlite or duckdb over
                            standard input and output, one JSON request per line; with call,
                            answer the one request on standard input
  --version                 Print the version";

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
        Some(other) => {
            eprintln!("reylo: unknown command {other}\n\n{USAGE}");
            2
        }
    };
    std::process::exit(code);
}
