//! The project file, `.reylo.yaml`: the settings the content mapper reads, parsed once here so
//! every host reads the same YAML the same way. A host that serves the project's queries itself
//! reads the file too: the top-level `env`, `url`, its connection, which the importer never reads,
//! and its own settings, the `api` section, which it parses itself.
//!
//! A host finds the file (beside the project's `package.json`), reads it and hands the text over;
//! what the file names outside itself (the environment, `.env` files, the database) is the host's
//! to resolve, and a `${VAR}` in a URL stays as written. There is no dialect key: the URL's scheme
//! names the dialect. A key the library does not know is an error, so a typo does not pass for a
//! default; the `api` and `rest` sections, which the importer does not read, are accepted as they
//! are, and a top-level `$schema` (the JSON Schema an editor validates the file
//! against, which `reylo schema --config` prints) is ignored.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The published schema an editor validates the file against; no host reads it.
    #[serde(rename = "$schema", default, skip_serializing)]
    pub schema: Option<serde_yaml::Value>,
    /// The connection a host that serves the queries runs them through, usually `${DATABASE_URL}`.
    /// Parsed for that host: the importer's type check never connects through it, and tells a file
    /// that sets it without `direct_url` so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The dev database the importer's type check reads, `${DATABASE_URL}` when absent: a
    /// connection that bypasses a pooler, since describing a statement through a transaction
    /// pooler can fail. The importer's alone: a host that serves the queries runs them through `url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_url: Option<String>,
    /// The `.env` files the importer and every other host read, relative to the project file
    /// (`.env.local`, then `.env`, when absent); the first that sets a variable wins, and the
    /// process environment wins over all of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,
    /// Whether a parameter the statement does not type is an error (`true`) or `unknown`. The key
    /// was `strict` until 2026-10-20.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_params: Option<bool>,
    /// Whether each statement the importer builds ends with a sqlcommenter comment naming its
    /// `.sql` file (`/*file='queries%2Fget_user.sql'*/`), which a database's monitoring reads as
    /// the statement's tags. On when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlcommenter: Option<bool>,
    /// The directory of the project's own files, beside the project file (`reylo` when absent):
    /// `schema.sql`, the migrations in `migrations/`, the lock `reylo-lock.yaml`, and the cache in
    /// `.cache/`, which the type check and the migrations keep there, out of git.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// `reylo migrate`'s settings; the section's presence opts the project in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate: Option<Migrate>,
    #[serde(default, skip_serializing)]
    pub rest: Option<serde_yaml::Value>,
    /// The settings of a host that serves the queries, which that host parses.
    #[serde(default, skip_serializing)]
    pub api: Option<serde_yaml::Value>,
}

/// `reylo migrate`'s settings: the database (the type check's connection when absent), a server to
/// make shadow databases on where the target's refuses `create database`, whether the project is
/// locked, and the strictness flags. Where the files are is the top-level `dir`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Migrate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_url: Option<String>,
    /// `linear` (locked) when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<History>,
    /// Compare comments on objects (on when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_comments: Option<bool>,
}

/// How a project's migrations are ordered. `linear`, the default: the project is locked, and
/// `reylo-lock.yaml` lists every migration in the order they run, so that two branches that each
/// add one conflict in git; a migration not in it is a draft only development runs. `loose`
/// (`reylo migrate unlock`): there is no lock, migrations run by name, and one older than one a
/// database applied still runs there, once replaying both orders reaches the same schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum History {
    Loose,
    Linear,
}

/// Why the file does not parse, at byte offsets in its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

pub fn parse(text: &str) -> Result<Project, ConfigError> {
    // An empty file, or one holding only comments, is a project with every default.
    if text.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with('#')) {
        return Ok(Project::default());
    }
    let masked = mask_placeholders(text);
    let mut project: Project = serde_yaml::from_str(&masked).map_err(|e| {
        let start = e.location().map(|l| l.index()).unwrap_or(0);
        // The message carries its own " at line L column C"; the host places the error itself.
        let message = e.to_string();
        let message = match message.rfind(" at line ") {
            Some(i) if message[i..].contains(" column ") => message[..i].to_string(),
            _ => message,
        };
        let end = text[start.min(text.len())..].find(['\n', ':']).map_or(text.len(), |n| start + n);
        // `strict` was renamed `strict_params` on 2026-10-20: the old key is unknown like any
        // other, and its error names the new one.
        // The `cli` section said which files `reylo generate` analyzed, and went with it in 0.8.0.
        let message = if message.starts_with("unknown field `strict`,") {
            "unknown field `strict`: the key is `strict_params` since 2026-10-20".to_string()
        } else if message.starts_with("unknown field `cli`,") {
            "unknown field `cli`: the section said which files reylo generate analyzed, and went with it in 0.8.0; delete it".to_string()
        } else {
            message
        };
        ConfigError { message: unmask_placeholders(message.trim_end()), start, end }
    })?;
    if masked != text {
        project.url = project.url.as_deref().map(unmask_placeholders);
        project.direct_url = project.direct_url.as_deref().map(unmask_placeholders);
        project.dir = project.dir.as_deref().map(unmask_placeholders);
        if let Some(m) = project.migrate.as_mut() {
            for value in [&mut m.url, &mut m.shadow_url].into_iter().flatten() {
                *value = unmask_placeholders(value);
            }
        }
        for item in project.env.iter_mut().flatten() {
            *item = unmask_placeholders(item);
        }
    }
    Ok(project)
}

/// The text with each `${VAR}` spelled `$<VAR>`. A bare `${VAR}` inside a flow mapping or
/// sequence is not YAML (its `{` opens a nested mapping), and the `api` section is often
/// written so (`jwt: { jwks: ${JWKS_URL} }`): the host that reads it interpolates before it parses,
/// but the importer keeps a URL's `${VAR}` as written. The spelling has the same length, so an
/// error's offsets still point into the text, and `unmask_placeholders` turns it back in what
/// the parse returns. A text that already holds `$<` is parsed as written.
fn mask_placeholders(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains("$<") || !text.contains("${") {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(respell(text, "${", '}', "$<", '>'))
}

fn unmask_placeholders(text: &str) -> String {
    respell(text, "$<", '>', "${", '}')
}

/// Each `<open>NAME<close>` with a variable name between, spelled with the other delimiters.
fn respell(text: &str, open: &str, close: char, to_open: &str, to_close: char) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(open) {
        out.push_str(&rest[..at]);
        let after = &rest[at + open.len()..];
        let name_len = after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len());
        if name_len > 0 && after[name_len..].starts_with(close) {
            out.push_str(to_open);
            out.push_str(&after[..name_len]);
            out.push(to_close);
            rest = &after[name_len + close.len_utf8()..];
        } else {
            out.push_str(open);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_library_keys_and_leaves_the_later_sections_alone() {
        let p = parse("url: ${DATABASE_URL}\ndirect_url: ${DIRECT_URL}\nenv: [.env]\nstrict_params: true\nsqlcommenter: false\napi:\n  routes: api\nrest:\n  anything: { x: 1 }\n").unwrap();
        assert_eq!(p.url.as_deref(), Some("${DATABASE_URL}"));
        assert_eq!(p.direct_url.as_deref(), Some("${DIRECT_URL}"));
        assert_eq!(p.env, Some(vec![".env".to_string()]));
        assert_eq!(p.strict_params, Some(true));
        assert_eq!(p.sqlcommenter, Some(false));
        assert!(p.api.is_some() && p.rest.is_some());
        // What a host reads back: the later sections are not its business.
        assert_eq!(serde_json::to_value(&p).unwrap(), serde_json::json!({ "url": "${DATABASE_URL}", "direct_url": "${DIRECT_URL}", "env": [".env"], "strict_params": true, "sqlcommenter": false }));
        assert_eq!(parse("").unwrap(), Project::default());
        assert_eq!(parse("# nothing yet\n").unwrap(), Project::default());
    }

    #[test]
    fn reads_the_migrate_section() {
        let p = parse("dir: db\nmigrate:\n  url: ${MIGRATE_URL}\n  history: loose\n").unwrap();
        assert_eq!(serde_json::to_value(&p).unwrap(), serde_json::json!({ "dir": "db", "migrate": { "url": "${MIGRATE_URL}", "history": "loose" } }));
        assert!(parse("migrate:\n  dir: db/migrations\n").is_err(), "the migrations' place is the top-level dir's");
        assert_eq!(parse("migrate:\n  history: loose\n").unwrap().migrate.and_then(|m| m.history), Some(History::Loose));
        assert_eq!(parse("migrate: {}\n").unwrap().migrate, Some(Migrate::default()));
        assert!(parse("migrate:\n  folder: x\n").is_err());
        assert!(parse("migrate:\n  history: strict\n").is_err());
    }

    #[test]
    fn a_key_the_library_does_not_know_is_an_error_at_its_place() {
        let text = "url: postgres://x/db\ndialect: sqlite\n";
        let e = parse(text).unwrap_err();
        assert!(e.message.starts_with("unknown field `dialect`"), "{}", e.message);
        assert_eq!(&text[e.start..e.end], "dialect");
        // The types section was removed on 2026-10-20: it is a key like any other the library
        // does not know (the loader refuses it before the engine reads the file, with its own
        // message).
        let text = "url: postgres://x/db\ntypes:\n  dates: temporal\n";
        let e = parse(text).unwrap_err();
        assert!(e.message.starts_with("unknown field `types`"), "{}", e.message);
        assert_eq!(&text[e.start..e.end], "types");
        let text = "cli:\n  include: [\"src/**/*.sql\"]\n";
        let e = parse(text).unwrap_err();
        assert_eq!(e.message, "unknown field `cli`: the section said which files reylo generate analyzed, and went with it in 0.8.0; delete it");
        assert_eq!(&text[e.start..e.end], "cli");
        let e = parse("strict_params: sometimes\n").unwrap_err();
        assert!(e.message.contains("invalid type"), "{}", e.message);
        // `strict` was renamed `strict_params` on 2026-10-20, with no alias: the old key is
        // unknown, and the error names the new one.
        let text = "url: postgres://x/db\nstrict: true\n";
        let e = parse(text).unwrap_err();
        assert_eq!(e.message, "unknown field `strict`: the key is `strict_params` since 2026-10-20");
        assert_eq!(&text[e.start..e.end], "strict");
    }

    #[test]
    fn a_framework_project_file_reads_with_its_schema_and_api_section() {
        let text = "$schema: https://example.test/reylo.schema.json\nurl: ${DATABASE_URL}\nenv: [.env.local, .env]\napi:\n  authenticators:\n    clerk:\n      jwt:\n        jwks: ${JWKS_URL}\n  codegen:\n    typescript: { out: web/src/api }\n";
        let p = parse(text).unwrap();
        assert_eq!(p.url.as_deref(), Some("${DATABASE_URL}"));
        assert!(p.schema.is_some() && p.api.is_some());
        // Neither `$schema` nor `api` is the host's to read back.
        assert_eq!(serde_json::to_value(&p).unwrap(), serde_json::json!({ "url": "${DATABASE_URL}", "env": [".env.local", ".env"] }));
    }

    #[test]
    fn a_placeholder_in_a_flow_mapping_parses_and_reads_back_as_written() {
        let text = "url: ${DATABASE_URL}\ndirect_url: \"${DIRECT_URL}\"\nenv: [${ENV_FILE}, .env]\napi:\n  authenticators:\n    clerk:\n      jwt: { from: { header: Authorization, scheme: Bearer }, jwks: ${JWKS_URL}, issuer: ${ISSUER} }\n";
        let p = parse(text).unwrap();
        assert_eq!(p.url.as_deref(), Some("${DATABASE_URL}"));
        assert_eq!(p.direct_url.as_deref(), Some("${DIRECT_URL}"));
        assert_eq!(p.env, Some(vec!["${ENV_FILE}".to_string(), ".env".to_string()]));
        assert_eq!(p.api.unwrap()["authenticators"]["clerk"]["jwt"]["jwks"].as_str(), Some("$<JWKS_URL>"), "the api section is its own host's to read");
        // The offsets of an error still point into the text as written.
        let text = "url: ${DATABASE_URL}\napi: { a: ${X} }\ndialect: x\n";
        let e = parse(text).unwrap_err();
        assert_eq!(&text[e.start..e.end], "dialect");
        // A text that writes `$<` itself is read as written.
        assert!(parse("url: $<x>\napi: { a: ${X} }\n").is_err());
    }
}
