//! The project file, `.reylo.yaml` (or `.reylo.yml`), and the connection it names, resolved as the
//! type check resolves them (src/project.ts): the file is `REYLO_CONFIG`, else the one beside the
//! nearest `package.json` at or above the working directory; with no `package.json` there, which no
//! project the type check reads lacks, the nearest project file at or above it. A `${VAR}` in a URL
//! comes from the process environment, then the `.env` files the project file names under `env`
//! (`.env.local`, then `.env`, beside it, by default); a `$(command)` in `direct_url` is replaced by
//! what the command prints. The type check's connection is `direct_url`, else `DATABASE_URL`.

use reylo_engine_core::config;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const PROJECT_FILES: &[&str] = &[".reylo.yaml", ".reylo.yml"];
const DEFAULT_ENV_FILES: &[&str] = &[".env.local", ".env"];
/// How long a `$(command)` in `direct_url` may run, as the type check allows it: a password
/// manager's prompt is answered in that time.
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The byte offset of a top-level `types:` key, the section removed on 2026-10-20.
fn types_key(text: &str) -> Option<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("types") {
            if rest.trim_start_matches([' ', '\t']).starts_with(':') {
                return Some(at);
            }
        }
        at += line.len();
    }
    None
}

pub struct Project {
    pub file: Option<PathBuf>,
    /// The directory a relative path in the project resolves against: the project file's, else the
    /// package's, else the working directory.
    pub root: PathBuf,
    /// The working directory, which paths in messages are written from.
    cwd: PathBuf,
    pub config: config::Project,
    env_files: Vec<PathBuf>,
    env: HashMap<String, String>,
}

/// The nearest directory at or above `from` that holds a `package.json`.
pub fn package_dir(from: &Path) -> Option<PathBuf> {
    from.ancestors().find(|d| d.join("package.json").exists()).map(Path::to_path_buf)
}

/// The project file for the working directory: `REYLO_CONFIG`, else the one beside the package's
/// `package.json`, else, outside any package, the nearest one at or above it.
pub fn find(cwd: &Path, env: &HashMap<String, String>) -> Option<PathBuf> {
    if let Some(file) = env.get("REYLO_CONFIG").filter(|f| !f.is_empty()) {
        return Some(cwd.join(file));
    }
    let beside = |d: &Path| PROJECT_FILES.iter().map(|name| d.join(name)).find(|p| p.exists());
    match package_dir(cwd) {
        Some(dir) => beside(&dir),
        None => cwd.ancestors().find_map(beside),
    }
}

/// The database a URL's scheme names, as the type check reads it (`dialectOf` in src/project.ts).
pub fn dialect_of(url: &str) -> Option<&'static str> {
    let scheme = url.split_once(':')?.0.to_ascii_lowercase();
    if !scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')) {
        return None;
    }
    match scheme.as_str() {
        "postgres" | "postgresql" => Some("postgres"),
        "mysql" | "mariadb" => Some("mysql"),
        "file" | "sqlite" => Some("sqlite"),
        "duckdb" => Some("duckdb"),
        _ => None,
    }
}

/// `file:./dev.db`, `file:///abs/dev.db`, `sqlite:./dev.db`, `:memory:` → a path, a relative one
/// resolved against `root` (`sqlitePath` in src/project.ts). A `file://` URL is read as Node's
/// `fileURLToPath` reads it, or refused as it refuses it.
pub fn sqlite_path(url: &str, root: &Path) -> Result<String, String> {
    if url == ":memory:" || url == "file::memory:" {
        return Ok(":memory:".into());
    }
    let raw = match url.strip_prefix("file://") {
        Some(rest) => file_url_path(rest, cfg!(windows)).map_err(|why| format!("Cannot read the SQLite URL {url}: {why}."))?,
        None => {
            let rest = url.strip_prefix("file:").or_else(|| url.strip_prefix("sqlite:")).unwrap_or(url);
            rest.strip_prefix("//").unwrap_or(rest).to_string()
        }
    };
    let path = Path::new(&raw);
    Ok(if path.is_absolute() { raw } else { root.join(path).to_string_lossy().into_owned() })
}

/// The path of a `file://` URL, `rest` being what follows `file://`: its host empty or
/// `localhost`, else on Windows a share (`\\host\path`); on Windows `/C:/x` is `C:\x`. Its
/// query and fragment are no part of the path.
fn file_url_path(rest: &str, windows: bool) -> Result<String, String> {
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (host, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, ""),
    };
    let host = if host.eq_ignore_ascii_case("localhost") { "" } else { host };
    if path.to_ascii_lowercase().contains("%2f") || (windows && path.to_ascii_lowercase().contains("%5c")) {
        return Err("its path encodes a separator".into());
    }
    let path = percent_decode(path);
    if !windows {
        if !host.is_empty() {
            return Err(format!("its host must be localhost or empty, not {host}"));
        }
        return Ok(path);
    }
    let path = path.replace('/', "\\");
    if !host.is_empty() {
        return Ok(format!("\\\\{host}{path}"));
    }
    let b = path.as_bytes();
    if b.len() >= 3 && b[0] == b'\\' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        Ok(path[1..].to_string())
    } else {
        Err("its path must be absolute, with a drive letter".into())
    }
}

fn percent_decode(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `NAME=value` lines of an env file, quotes around a value dropped.
pub fn read_env_file(path: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else { return HashMap::new() };
    let mut out = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((name, value)) = line.split_once('=') else { continue };
        let name = name.trim();
        if name.is_empty() || !name.chars().enumerate().all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())) {
            continue;
        }
        let value = value.trim();
        let value = match (value.chars().next(), value.chars().last()) {
            (Some(a @ ('"' | '\'')), Some(b)) if a == b && value.len() >= 2 => &value[1..value.len() - 1],
            _ => value,
        };
        out.insert(name.to_string(), value.to_string());
    }
    out
}

/// The 1-based line and column of a byte offset.
fn place(text: &str, offset: usize) -> String {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    format!("{line}:{column}")
}

impl Project {
    pub fn resolve(cwd: &Path, env: HashMap<String, String>) -> Result<Project, String> {
        let file = find(cwd, &env);
        let root = file.as_deref().and_then(Path::parent).map(Path::to_path_buf).or_else(|| package_dir(cwd)).unwrap_or_else(|| cwd.to_path_buf());
        let mut config = config::Project::default();
        if let Some(path) = &file {
            let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", shown(cwd, path)))?;
            if let Some(at) = types_key(&text) {
                return Err(format!("{}:{}: types: the types section was removed on 2026-10-20. Reylo now gives every value one type on every driver, whatever the driver's settings: a 64-bit integer is a BigInt, a decimal its exact text, a timestamptz its ISO 8601 text in UTC, a date and a timestamp their text, JSON parsed and binary data a Uint8Array. Delete the section.", shown(cwd, path), place(&text, at)));
            }
            config = config::parse(&text).map_err(|e| format!("{}:{}: {}", shown(cwd, path), place(&text, e.start), e.message))?;
        }
        // Both names beside one another: whichever were read, edits to the other would do nothing.
        let both: Vec<PathBuf> = PROJECT_FILES.iter().map(|name| root.join(name)).collect();
        if !env.contains_key("REYLO_CONFIG") && both.iter().all(|p| p.exists()) {
            return Err(format!("{} are both project files; keep one.", both.iter().map(|p| shown(cwd, p)).collect::<Vec<_>>().join(" and ")));
        }
        let env_files = config.env.clone().unwrap_or_else(|| DEFAULT_ENV_FILES.iter().map(|s| s.to_string()).collect()).iter().map(|f| root.join(f)).collect();
        Ok(Project { file, root, cwd: cwd.to_path_buf(), config, env_files, env })
    }

    /// Whether a parameter the statement does not type is an error: unless the file says `strict_params: false`.
    pub fn strict_params(&self) -> bool {
        self.config.strict_params.unwrap_or(true)
    }

    /// That the file sets `url`, which the type check does not read, and no `direct_url`.
    pub fn notice(&self) -> Option<String> {
        (self.config.url.is_some() && self.config.direct_url.is_none()).then(|| format!("{}: Reylo does not read url; set direct_url to the dev database Reylo reads your types from (it uses DATABASE_URL meanwhile)", self.shown_file()))
    }

    /// What a run with no connection is told: where to set one.
    pub fn missing_url(&self) -> String {
        if self.config.url.is_some() && self.config.direct_url.is_none() {
            format!("No database URL: {} sets url, which Reylo does not read; set direct_url there, or DATABASE_URL in the environment, .env.local or .env.", self.shown_file())
        } else {
            format!("No database URL: set DATABASE_URL in the environment, .env.local or .env, or \"direct_url\" in {}.", self.shown_file())
        }
    }

    /// Whether `direct_url` runs a command, which [`Project::type_check_url`] runs again when the
    /// database refuses the login, as a short-lived token's command must.
    pub fn url_runs_command(&self) -> bool {
        self.config.direct_url.as_deref().is_some_and(|u| url_parts(u).is_ok_and(|parts| parts.iter().any(|p| matches!(p, Part::Command(_)))))
    }

    /// A variable from the environment, else the first env file that sets it.
    pub fn lookup(&self, name: &str) -> Option<String> {
        if let Some(v) = self.env.get(name) {
            return Some(v.clone());
        }
        self.env_files.iter().find_map(|p| read_env_file(p).get(name).cloned())
    }

    fn shown_file(&self) -> String {
        self.file.as_deref().map(|f| shown(&self.cwd, f)).unwrap_or_else(|| ".reylo.yaml".into())
    }

    /// Each `${VAR}` replaced, or the variables nothing sets: a `migrate` setting's value.
    pub fn expand(&self, key: &str, text: &str) -> Result<String, String> {
        let mut missing = Vec::new();
        let out = self.substitute(text, &mut missing);
        if missing.is_empty() { Ok(out) } else { Err(self.unset(key, &missing)) }
    }

    /// `text` with each `${VAR}` replaced, the variables nothing sets added to `missing`, and each
    /// `${VAR:-fallback}` by its variable, or where that is unset or empty by its fallback, read the
    /// same way, as a shell reads one. A `${` that opens neither stays as written.
    fn substitute(&self, text: &str, missing: &mut Vec<String>) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find("${") {
            out.push_str(&rest[..at]);
            let after = &rest[at + 2..];
            let name_len = after.find(|c: char| !(c == '_' || c.is_ascii_alphanumeric())).unwrap_or(after.len());
            let name = &after[..name_len];
            let tail = &after[name_len..];
            let body = tail.strip_prefix(":-").filter(|_| !name.is_empty());
            let closes = body.and_then(fallback_end);
            if !name.is_empty() && tail.starts_with('}') {
                match self.lookup(name) {
                    Some(v) => out.push_str(&v),
                    None => missing.push(format!("${{{name}}}")),
                }
                rest = &tail[1..];
            } else if let (Some(body), Some(end)) = (body, closes) {
                match self.lookup(name).filter(|v| !v.is_empty()) {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&self.substitute(&body[..end], missing)),
                }
                rest = &body[end + 1..];
            } else {
                out.push_str("${");
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }

    /// The message for a key that names variables nothing sets.
    fn unset(&self, key: &str, missing: &[String]) -> String {
        let files = self.env_files.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect::<Vec<_>>().join(" nor ");
        format!("{}: \"{key}\" names {}, which neither the environment nor {files} sets.", self.shown_file(), missing.join(", "))
    }

    /// The type check's connection: `direct_url`, its `$(command)`s run, else `DATABASE_URL`.
    pub fn type_check_url(&self) -> Result<Option<String>, String> {
        let Some(written) = &self.config.direct_url else { return Ok(self.lookup("DATABASE_URL")) };
        let parts = url_parts(written).map_err(|open| format!("{}: \"direct_url\" opens $( at column {} and never closes it.", self.shown_file(), open + 1))?;
        let encoded = in_userinfo(&parts);
        // Every `${VAR}` of the text first: a project that cannot be used anyway runs no command,
        // so that it asks for no password.
        let mut missing = Vec::new();
        let texts: Vec<Option<String>> = parts.iter().map(|part| match part {
            Part::Text(t) => Some(self.substitute(t, &mut missing)),
            Part::Command(_) => None,
        }).collect();
        if !missing.is_empty() {
            return Err(self.unset("direct_url", &missing));
        }
        let mut url = String::new();
        for (k, part) in parts.iter().enumerate() {
            match part {
                Part::Text(_) => url.push_str(texts[k].as_deref().unwrap_or_default()),
                Part::Command(c) => {
                    let value = self.command_output(c).map_err(|why| format!("{}: \"direct_url\" runs `{c}`, {why}.", self.shown_file()))?;
                    url.push_str(&if encoded.contains(&k) { percent_encode(&value) } else { value });
                }
            }
        }
        Ok(Some(url))
    }

    fn command_output(&self, command: &str) -> Result<String, String> {
        let mut env: HashMap<String, String> = HashMap::new();
        for path in self.env_files.iter().rev() {
            env.extend(read_env_file(path));
        }
        env.extend(self.env.clone());
        let mut c = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.arg("/C").arg(command);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.arg("-c").arg(command);
            c
        };
        let mut child = c
            .current_dir(&self.root)
            .env_clear()
            .envs(&env)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("which could not start: {e}"))?;
        // Both streams are read as the command writes them, so that one that writes more than a
        // pipe holds never blocks before it exits.
        let drain = |stream: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                if let Some(mut stream) = stream {
                    let _ = std::io::Read::read_to_end(&mut stream, &mut bytes);
                }
                bytes
            })
        };
        let stdout = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>));
        let stderr = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>));
        let started = std::time::Instant::now();
        while child.try_wait().map_err(|e| format!("which could not be waited for: {e}"))?.is_none() {
            if started.elapsed() > COMMAND_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("which did not finish within {} s", COMMAND_TIMEOUT.as_secs()));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let status = child.wait().map_err(|e| format!("which could not be waited for: {e}"))?;
        let out = std::process::Output { status, stdout: stdout.join().unwrap_or_default(), stderr: stderr.join().unwrap_or_default() };
        if !out.status.success() {
            let said = String::from_utf8_lossy(&out.stderr).lines().map(str::trim).find(|l| !l.is_empty()).map(|l| format!(": {}", l.chars().take(300).collect::<String>())).unwrap_or_default();
            return Err(match out.status.code() {
                Some(code) => format!("which exited with code {code}{said}"),
                None => format!("which ended on a signal{said}"),
            });
        }
        let value = String::from_utf8_lossy(&out.stdout).trim_end_matches(['\r', '\n']).to_string();
        if value.is_empty() {
            Err("which printed nothing".into())
        } else {
            Ok(value)
        }
    }
}

/// A path as messages write it: from the working directory, with `/`, where it is under it.
fn shown(cwd: &Path, path: &Path) -> String {
    match path.strip_prefix(cwd) {
        Ok(rest) if !rest.as_os_str().is_empty() => rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"),
        _ => path.to_string_lossy().into_owned(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Part {
    Text(String),
    Command(String),
}

/// `direct_url` cut into its text and its `$(command)`s. A command ends at the `)` that balances its
/// `$(`, outside quotes, as in a shell; a `$(` left open is the offset it opens at.
fn url_parts(written: &str) -> Result<Vec<Part>, usize> {
    let b = written.as_bytes();
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < b.len() {
        if !(b[i] == b'$' && b.get(i + 1) == Some(&b'(')) {
            let width = written[i..].chars().next().map_or(1, char::len_utf8);
            text.push_str(&written[i..i + width]);
            i += width;
            continue;
        }
        let mut depth = 1;
        let mut quote: Option<u8> = None;
        let mut j = i + 2;
        while j < b.len() && depth > 0 {
            let c = b[j];
            match quote {
                Some(q) if c == q => quote = None,
                Some(b'"') if c == b'\\' => j += 1,
                Some(_) => {}
                None if c == b'\'' || c == b'"' => quote = Some(c),
                None if c == b'\\' => j += 1,
                None if c == b'(' => depth += 1,
                None if c == b')' => depth -= 1,
                None => {}
            }
            j += 1;
        }
        if depth > 0 {
            return Err(i);
        }
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(&mut text)));
        }
        parts.push(Part::Command(written[i + 2..j - 1].to_string()));
        i = j;
    }
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    Ok(parts)
}

/// The commands whose output lands in the URL's user or password, between `://` and the `@` that
/// ends them: their output is percent-encoded there, as a password with `/`, `@` or `?` in it would
/// otherwise end the user part early.
fn in_userinfo(parts: &[Part]) -> Vec<usize> {
    let mut skeleton = String::new();
    let mut placed = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        match part {
            Part::Text(t) => skeleton.push_str(t),
            Part::Command(_) => {
                placed.push((skeleton.len(), k));
                skeleton.push('\0');
            }
        }
    }
    let Some(scheme) = skeleton.find("://") else { return vec![] };
    let start = scheme + 3;
    let authority_end = skeleton[start..].find(['/', '?', '#']).map_or(skeleton.len(), |e| start + e);
    let Some(userinfo_end) = skeleton[..authority_end].rfind('@') else { return vec![] };
    placed.into_iter().filter(|(at, _)| *at >= start && *at < userinfo_end).map(|(_, k)| k).collect()
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Where the `}` that closes a `${VAR:-` falls in what follows it, past any `${…}` inside.
fn fallback_end(body: &str) -> Option<usize> {
    let mut depth = 1;
    let bytes = body.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'$' && bytes.get(at + 1) == Some(&b'{') {
            depth += 1;
            at += 1;
        } else if bytes[at] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
        at += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_commands_out_of_a_url() {
        assert_eq!(url_parts("postgres://u:$(pass show db)@h/db").unwrap(), vec![Part::Text("postgres://u:".into()), Part::Command("pass show db".into()), Part::Text("@h/db".into())]);
        assert_eq!(url_parts("x$(echo ')')y").unwrap(), vec![Part::Text("x".into()), Part::Command("echo ')'".into()), Part::Text("y".into())]);
        assert_eq!(url_parts("x$(echo").unwrap_err(), 1);
        assert_eq!(in_userinfo(&url_parts("postgres://u:$(p)@h/$(d)").unwrap()), vec![1]);
        assert_eq!(percent_encode("a/b@c"), "a%2Fb%40c");
    }

    #[test]
    fn finds_the_project_file_as_the_type_check_does() {
        let root = std::env::temp_dir().join(format!("reylo-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sub = root.join("app/src/queries");
        std::fs::create_dir_all(&sub).unwrap();
        // Outside any package, the nearest project file at or above.
        std::fs::write(root.join(".reylo.yaml"), "").unwrap();
        assert_eq!(find(&sub, &HashMap::new()), Some(root.join(".reylo.yaml")));
        // Inside one, the file beside its package.json alone, as src/project.ts finds it.
        std::fs::write(root.join("app/package.json"), "{}").unwrap();
        assert_eq!(find(&sub, &HashMap::new()), None);
        std::fs::write(root.join("app/.reylo.yml"), "").unwrap();
        assert_eq!(find(&sub, &HashMap::new()), Some(root.join("app/.reylo.yml")));
        assert_eq!(Project::resolve(&sub, HashMap::new()).unwrap().root, root.join("app"));
        // REYLO_CONFIG names it outright, from the working directory.
        assert_eq!(find(&sub, &HashMap::from([("REYLO_CONFIG".to_string(), "x.yaml".to_string())])), Some(sub.join("x.yaml")));
        // Both names beside one package.json are one too many.
        std::fs::write(root.join("app/.reylo.yaml"), "").unwrap();
        assert!(Project::resolve(&sub, HashMap::new()).err().unwrap().ends_with("are both project files; keep one."));
        std::fs::remove_file(root.join("app/.reylo.yaml")).unwrap();
        std::fs::write(root.join("app/.reylo.yml"), "strict_params: false\ntypes:\n  int8: number\n").unwrap();
        assert!(Project::resolve(&sub, HashMap::new()).err().unwrap().contains(":2:1: types: the types section was removed on 2026-10-20."));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reads_the_database_from_a_url() {
        assert_eq!(dialect_of("postgresql://u@h/db"), Some("postgres"));
        assert_eq!(dialect_of("MySQL://root@h"), Some("mysql"));
        assert_eq!(dialect_of("mariadb://root@h"), Some("mysql"));
        assert_eq!(dialect_of("file:./app.db"), Some("sqlite"));
        assert_eq!(dialect_of("duckdb::memory:"), Some("duckdb"));
        assert_eq!(dialect_of("http://h"), None);
        assert_eq!(dialect_of(":memory:"), None);
        assert_eq!(sqlite_path(":memory:", Path::new("/app")).unwrap(), ":memory:");
        if cfg!(unix) {
            let root = Path::new("/app");
            assert_eq!(sqlite_path("file:./dev.db", root).unwrap(), "/app/./dev.db");
            assert_eq!(sqlite_path("sqlite:dev.db", root).unwrap(), "/app/dev.db");
            assert_eq!(sqlite_path("file:///var/my%20db.db", root).unwrap(), "/var/my db.db");
            assert_eq!(sqlite_path("file://localhost/var/app.db", root).unwrap(), "/var/app.db");
            assert!(sqlite_path("file://db.example/var/app.db", root).is_err());
        }
    }

    #[test]
    fn a_file_url_is_read_as_node_reads_it_on_each_platform() {
        assert_eq!(file_url_path("/var/app.db", false).unwrap(), "/var/app.db");
        assert_eq!(file_url_path("localhost/var/app.db?mode=ro", false).unwrap(), "/var/app.db");
        assert!(file_url_path("host/var/app.db", false).is_err());
        assert!(file_url_path("/var/a%2Fb.db", false).is_err());
        assert_eq!(file_url_path("/C:/app/my%20app.db", true).unwrap(), "C:\\app\\my app.db");
        assert_eq!(file_url_path("localhost/C:/app.db", true).unwrap(), "C:\\app.db");
        assert_eq!(file_url_path("server/share/app.db", true).unwrap(), "\\\\server\\share\\app.db");
        assert!(file_url_path("/app.db", true).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_direct_url_runs_no_command_until_every_variable_is_set_and_reads_long_output() {
        let dir = std::env::temp_dir().join(format!("reylo-direct-url-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A variable nothing sets after the command: the command never runs.
        std::fs::write(dir.join(".reylo.yaml"), "direct_url: postgres://u:$(touch ran && echo pw)@${UNSET_HOST_X}/app\n").unwrap();
        let p = Project::resolve(&dir, HashMap::new()).unwrap();
        assert!(p.type_check_url().unwrap_err().contains("${UNSET_HOST_X}"));
        assert!(!dir.join("ran").exists());
        // A command that writes more than a pipe holds to each stream before its answer finishes.
        std::fs::write(dir.join(".reylo.yaml"), "direct_url: postgres://u:$(head -c 200000 /dev/zero | tr '\\0' x >&2; head -c 200000 /dev/zero >/dev/null; echo pw)@h/app\n").unwrap();
        let p = Project::resolve(&dir, HashMap::new()).unwrap();
        assert_eq!(p.type_check_url().unwrap().as_deref(), Some("postgres://u:pw@h/app"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_env_files() {
        let dir = std::env::temp_dir().join(format!("reylo-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "export A=1\nB='two words'\n# C=3\nD=\"x=y\"\n").unwrap();
        let vars = read_env_file(&dir.join(".env"));
        assert_eq!(vars.get("A").map(String::as_str), Some("1"));
        assert_eq!(vars.get("B").map(String::as_str), Some("two words"));
        assert_eq!(vars.get("C"), None);
        assert_eq!(vars.get("D").map(String::as_str), Some("x=y"));
        std::fs::write(dir.join(".reylo.yaml"), "strict_params: false\n").unwrap();
        let p = Project::resolve(&dir, HashMap::new()).unwrap();
        assert!(p.expand("url", "${A}-${MISSING_VAR_X}").unwrap_err().contains("${MISSING_VAR_X}"));
        assert_eq!(p.expand("url", "db-${A}").unwrap(), "db-1");
        // A fallback is read only where the variable is unset or empty, and may name another.
        assert_eq!(p.expand("url", "${A:-x}").unwrap(), "1");
        assert_eq!(p.expand("url", "${MISSING_VAR_X:-x}").unwrap(), "x");
        assert_eq!(p.expand("url", "${MISSING_VAR_X:-}").unwrap(), "");
        assert_eq!(p.expand("url", "a${MISSING_VAR_X:-${A}}b").unwrap(), "a1b");
        assert_eq!(p.expand("url", "${MISSING_VAR_X:-${MISSING_VAR_Y:-z}}").unwrap(), "z");
        assert!(p.expand("url", "${MISSING_VAR_X:-${MISSING_VAR_Y}}").unwrap_err().contains("${MISSING_VAR_Y}"));
        assert_eq!(p.expand("url", "${MISSING_VAR_X:-unclosed").unwrap(), "${MISSING_VAR_X:-unclosed");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
