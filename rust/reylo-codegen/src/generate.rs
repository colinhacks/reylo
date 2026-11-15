//! The modules `#[reylo::query]` expands to, written to files: `reylo generate rust-sqlx` writes a
//! `.rs` file beside each `.sql` file, or every module to one file with `--out`, and a build script
//! (`reylo_build::rust_sqlx()`) writes one file into Cargo's `OUT_DIR`. A file's module is named
//! after it, so `get_user.sql` is `get_user`, holding `Query`, `Row` and `run`.
//!
//! A module written beside its crate's sources checks, as the crate compiles, that its `.sql` file
//! still reads as the text it was written from: an edited query fails the build until it is
//! generated again, with no database.

use crate::{analysis, files, items, Dates, Decimals, Options};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// The first line of every file the generator writes, which it looks for before it writes over one.
const MARK: &str = "// Written by `reylo generate rust-sqlx`";

/// The files to read and where to write their modules. Made by [`rust_sqlx`].
#[derive(Clone, Debug, Default)]
pub struct Generator {
    inputs: Vec<String>,
    out: Option<PathBuf>,
    build_script: bool,
}

/// What stopped a generation: every message, one per line, each naming its file.
#[derive(Clone, PartialEq, Eq)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The message itself, so that a build script's `.unwrap()` prints it as written.
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// A file the generator wrote, and whether its text changed: one it would write as it already
/// reads is left alone, so that its time stays and Cargo compiles nothing again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub path: PathBuf,
    pub changed: bool,
}

/// The modules of sqlx queries, as `#[reylo::query]` writes them.
pub fn rust_sqlx() -> Generator {
    Generator::default()
}

impl Generator {
    /// A `.sql` file, a directory searched for them, or a glob (`src/**/*.sql`), from the working
    /// directory, or from the crate's directory in a build script. With none, the crate's `src/`.
    pub fn input(mut self, path: impl Into<String>) -> Self {
        self.inputs.push(path.into());
        self
    }

    /// One file for every module, each directory under the inputs a module of its own. In a build
    /// script it is a name in `OUT_DIR`, `queries.rs` where none is given.
    pub fn out(mut self, file: impl Into<PathBuf>) -> Self {
        self.out = Some(file.into());
        self
    }

    /// For a build script: inputs read from the crate's directory, the one file written into
    /// `OUT_DIR`, and Cargo told which files to watch.
    pub fn in_build_script(mut self) -> Self {
        self.build_script = true;
        self
    }

    /// Reads every input, analyzes each statement and writes the modules: every file, or none
    /// where any statement fails, with every failure in the error.
    pub fn run(self) -> Result<Vec<Written>, Error> {
        let base = if self.build_script {
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").map_err(|_| Error("CARGO_MANIFEST_DIR is not set: reylo_build runs in a build script".into()))?)
        } else {
            std::env::current_dir().map_err(|e| Error(format!("cannot read the working directory: {e}")))?
        };
        let inputs = if !self.inputs.is_empty() {
            self.inputs.clone()
        } else if self.build_script {
            vec!["src".to_string()]
        } else {
            default_inputs(&base)?
        };
        let mut found = Vec::new();
        let mut errors = Vec::new();
        for input in &inputs {
            match files::sql_files(input, &base) {
                Ok(files) => found.extend(files),
                Err(message) => errors.push(message),
            }
        }
        if !errors.is_empty() {
            return Err(Error(errors.join("\n")));
        }
        found.sort();
        found.dedup();
        if found.is_empty() {
            return Err(Error(format!("no .sql file under {}", inputs.join(", "))));
        }

        let mut modules = Vec::new();
        let mut options = BTreeMap::new();
        for file in &found {
            let shown = shown(&base, file);
            match module(file, &shown, &mut options) {
                Ok((name, tokens)) => modules.push(Module { file: file.clone(), name, tokens }),
                Err(message) => errors.push(message),
            }
        }
        if !errors.is_empty() {
            return Err(Error(errors.join("\n")));
        }

        if self.build_script {
            let out_dir = PathBuf::from(std::env::var("OUT_DIR").map_err(|_| Error("OUT_DIR is not set: reylo_build runs in a build script".into()))?);
            let out = out_dir.join(self.out.clone().unwrap_or_else(|| "queries.rs".into()));
            let written = write(&out, &one_file(&modules, &out, true)?)?;
            for input in &inputs {
                let dir = if files::is_glob(input) { files::glob_root(input) } else { input.as_str() };
                println!("cargo:rerun-if-changed={}", base.join(dir).display());
            }
            for module in &modules {
                println!("cargo:rerun-if-changed={}", module.file.display());
            }
            let mut watched: Vec<PathBuf> = modules.iter().flat_map(|m| analysis::watched(&package_dir(&m.file))).collect();
            watched.sort();
            watched.dedup();
            for file in watched {
                println!("cargo:rerun-if-changed={}", file.display());
            }
            println!("cargo:rerun-if-env-changed=DATABASE_URL");
            return Ok(vec![written]);
        }
        match &self.out {
            Some(out) => {
                let out = base.join(out);
                Ok(vec![write(&out, &one_file(&modules, &out, false)?)?])
            }
            None => modules.iter().map(|m| write(&m.file.with_extension("rs"), &beside(m)?)).collect(),
        }
    }
}

/// A statement's module: its name and its items.
struct Module {
    file: PathBuf,
    name: String,
    tokens: TokenStream,
}

/// The `src/` of the crate the working directory is in, found from the nearest `Cargo.toml`, and
/// at a workspace's root, every member's.
fn default_inputs(cwd: &Path) -> Result<Vec<String>, Error> {
    let Some(manifest) = cwd.ancestors().map(|d| d.join("Cargo.toml")).find(|m| m.is_file()) else {
        return Err(Error("no Cargo.toml here or above: name the .sql files, the directories or a glob to read".into()));
    };
    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    let dir = manifest.parent().unwrap_or(cwd);
    if text.lines().any(|line| line.trim() == "[package]") {
        return Ok(vec![dir.join("src").to_string_lossy().into_owned()]);
    }
    let metadata = cargo_metadata(&manifest).ok_or_else(|| Error(format!("cannot read the members of the workspace {}: name the .sql files, the directories or a glob to read", manifest.display())))?;
    let members: Vec<String> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["manifest_path"].as_str())
        .map(|m| Path::new(m).parent().unwrap_or(dir).join("src"))
        .filter(|src| src.is_dir())
        .map(|src| src.to_string_lossy().into_owned())
        .collect();
    if members.is_empty() {
        return Err(Error(format!("no member of the workspace {} has a src/ directory", manifest.display())));
    }
    Ok(members)
}

/// One statement's module, or the messages that say why it cannot be written, each naming the
/// file, and the line and column where the engine places it.
fn module(file: &Path, shown: &str, options: &mut BTreeMap<PathBuf, Options>) -> Result<(String, TokenStream), String> {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = module_name(&stem).ok_or_else(|| format!("{shown}: {stem:?} is no Rust module's name: rename the file in snake_case, as get_user.sql"))?;
    let sql = std::fs::read_to_string(file).map_err(|e| format!("{shown}: cannot read it: {e}"))?;
    let package = package_dir(file);
    let (dialect, analysis) = analysis::analyze(&package, &sql).map_err(|message| format!("{shown}: {message}"))?;
    if let Some(diagnostics) = analysis["diagnostics"].as_array().filter(|d| !d.is_empty()) {
        return Err(diagnostics.iter().map(|d| diagnostic(shown, &sql, d)).collect::<Vec<_>>().join("\n"));
    }
    let options = *options.entry(package.clone()).or_insert_with(|| crate_options(&package));
    let tokens = items(dialect, options, &analysis, Span::call_site()).map_err(|e| format!("{shown}: {e}"))?;
    Ok((name, tokens))
}

/// A file's stem as a module's name: a Rust identifier, written raw where it is a keyword.
fn module_name(stem: &str) -> Option<String> {
    let valid = !stem.is_empty() && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !stem.starts_with(|c: char| c.is_ascii_digit());
    valid.then(|| if syn::parse_str::<syn::Ident>(stem).is_ok() { stem.to_string() } else { format!("r#{stem}") })
}

/// `file:line:column: message`, the place the engine gives as a byte offset into the statement.
fn diagnostic(shown: &str, sql: &str, d: &Value) -> String {
    let message = d["message"].as_str().unwrap_or("the statement cannot be analyzed");
    let start = d["start"].as_u64().unwrap_or(0) as usize;
    let before = &sql[..start.min(sql.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    format!("{shown}:{line}:{column}: {message}")
}

/// The crate a file belongs to: the nearest directory above it with a `Cargo.toml`, whose project
/// file and `.env` files name the database, as for the macro.
fn package_dir(file: &Path) -> PathBuf {
    let parent = file.parent().unwrap_or(file);
    parent.ancestors().find(|d| d.join("Cargo.toml").is_file()).unwrap_or(parent).to_path_buf()
}

/// The crates the modules may name for dates and times, uuids and decimals: the features the crate
/// turns on in its `reylo` dependency, as the macro reads them from its own.
fn crate_options(package: &Path) -> Options {
    let manifest = package.join("Cargo.toml");
    let Some(metadata) = cargo_metadata(&manifest) else { return Options::default() };
    let same = |a: &str| std::fs::canonicalize(a).ok() == std::fs::canonicalize(&manifest).ok();
    let Some(own) = metadata["packages"].as_array().into_iter().flatten().find(|p| p["manifest_path"].as_str().is_some_and(same)) else { return Options::default() };
    let features: Vec<&str> = own["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["rename"].as_str().unwrap_or(d["name"].as_str().unwrap_or_default()) == "reylo" || matches!(d["name"].as_str(), Some("reylo" | "reylo-sqlx")))
        .flat_map(|d| d["features"].as_array().into_iter().flatten().filter_map(Value::as_str))
        .collect();
    let has = |name: &str| features.contains(&name);
    Options {
        dates: if has("chrono") {
            Some(Dates::Chrono)
        } else if has("time") {
            Some(Dates::Time)
        } else {
            None
        },
        uuid: has("uuid"),
        decimals: if has("rust_decimal") {
            Some(Decimals::RustDecimal)
        } else if has("bigdecimal") {
            Some(Decimals::BigDecimal)
        } else {
            None
        },
    }
}

/// `cargo metadata` of the workspace `manifest` is in, without its dependencies' own: the cargo
/// running a build script, else the one on the PATH.
fn cargo_metadata(manifest: &Path) -> Option<Value> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(cargo).args(["metadata", "--no-deps", "--format-version", "1", "--offline", "--manifest-path"]).arg(manifest).stderr(std::process::Stdio::null()).output().ok()?;
    output.status.success().then(|| serde_json::from_slice(&output.stdout).ok()).flatten()
}

/// A module beside its `.sql` file: its items, after the check that the file still reads as the
/// text they were written from.
fn beside(module: &Module) -> Result<String, Error> {
    let file_name = module.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let check = check(&module.file, &file_name)?;
    let tokens = &module.tokens;
    let header = format!("{MARK} from {file_name}: edit the query and run it again, never this file.\n\n");
    format_file(header, quote!(#check #tokens))
}

/// Every module in one file, a module per directory under the inputs' common directory. A build
/// script's names each `.sql` file by its full path and needs no check, since Cargo runs it again
/// when one changes.
fn one_file(modules: &[Module], out: &Path, build_script: bool) -> Result<String, Error> {
    let common = common_dir(modules.iter().map(|m| m.file.parent().unwrap_or(&m.file)));
    #[derive(Default)]
    struct Node {
        dirs: BTreeMap<String, Node>,
        queries: BTreeMap<String, TokenStream>,
    }
    let mut root = Node::default();
    for module in modules {
        let rel = module.file.parent().and_then(|p| p.strip_prefix(&common).ok()).unwrap_or(Path::new(""));
        let mut node = &mut root;
        for part in rel.components() {
            let Component::Normal(part) = part else { continue };
            let part = part.to_string_lossy().into_owned();
            let name = module_name(&part).ok_or_else(|| Error(format!("{}: the directory {part:?} is no Rust module's name: rename it in snake_case", module.file.display())))?;
            node = node.dirs.entry(name).or_default();
        }
        let tokens = &module.tokens;
        let body = if build_script {
            quote!(#tokens)
        } else {
            let check = check(&module.file, &relative(out.parent().unwrap_or(Path::new("")), &module.file))?;
            quote!(#check #tokens)
        };
        node.queries.insert(module.name.clone(), body);
    }
    fn render(node: &Node, at: &Path) -> Result<TokenStream, Error> {
        let mut items = Vec::new();
        for (name, child) in &node.dirs {
            if node.queries.contains_key(name) {
                return Err(Error(format!("{}: both a directory and a .sql file are named {name}, which would be two modules of one name", at.display())));
            }
            let ident = syn::parse_str::<syn::Ident>(name).map_err(|e| Error(e.to_string()))?;
            let inner = render(child, &at.join(name))?;
            items.push(quote!(pub mod #ident { #inner }));
        }
        for (name, body) in &node.queries {
            let ident = syn::parse_str::<syn::Ident>(name).map_err(|e| Error(e.to_string()))?;
            items.push(quote!(pub mod #ident { #body }));
        }
        Ok(quote!(#(#items)*))
    }
    let tokens = render(&root, &common)?;
    let from = if build_script { common.display().to_string() } else { relative(out.parent().unwrap_or(Path::new("")), &common) };
    let header = format!("{MARK} from the .sql files under {from}: edit the queries and run it again, never this file.\n\n");
    format_file(header, tokens)
}

/// `const _: () = assert!(…)`: the crate fails to compile, saying why, once the `.sql` file at
/// `include` (from the generated file) no longer reads as the text it was written from.
fn check(file: &Path, include: &str) -> Result<TokenStream, Error> {
    let sql = std::fs::read_to_string(file).map_err(|e| Error(format!("cannot read {}: {e}", file.display())))?;
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let message = format!("{name} changed after its module was written: run `reylo generate rust-sqlx` again");
    Ok(quote! {
        const _: () = assert!(reylo::__private::same_text(include_str!(#include), #sql), #message);
    })
}

fn format_file(header: String, tokens: TokenStream) -> Result<String, Error> {
    let mut file: syn::File = syn::parse2(tokens).map_err(|e| Error(format!("the generated module does not parse: {e}")))?;
    crate::readable::readable(&mut file);
    Ok(header + &prettyplease::unparse(&file))
}

/// Writes `text` to `path` unless it already reads so; refuses to write over a file the generator
/// did not write.
fn write(path: &Path, text: &str) -> Result<Written, Error> {
    match std::fs::read_to_string(path) {
        Ok(old) if old == text => return Ok(Written { path: path.to_path_buf(), changed: false }),
        Ok(old) if !old.starts_with(MARK) => return Err(Error(format!("{} exists and reylo generate did not write it: move it, or rename the .sql file", path.display()))),
        _ => {}
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error(format!("cannot make {}: {e}", dir.display())))?;
    }
    std::fs::write(path, text).map_err(|e| Error(format!("cannot write {}: {e}", path.display())))?;
    Ok(Written { path: path.to_path_buf(), changed: true })
}

/// The deepest directory every one of `dirs` is in.
fn common_dir<'a>(mut dirs: impl Iterator<Item = &'a Path>) -> PathBuf {
    let Some(first) = dirs.next() else { return PathBuf::new() };
    let mut common = first.to_path_buf();
    for dir in dirs {
        while !dir.starts_with(&common) {
            if !common.pop() {
                return PathBuf::new();
            }
        }
    }
    common
}

/// `to` from the directory `from`, with `/` between its parts, as `include_str!` reads it on
/// every system.
fn relative(from: &Path, to: &Path) -> String {
    let common = common_dir([from, to].into_iter());
    let ups = from.strip_prefix(&common).map_or(0, |rest| rest.components().count());
    let down = to.strip_prefix(&common).unwrap_or(to);
    let mut parts: Vec<String> = std::iter::repeat("..".to_string()).take(ups).collect();
    parts.extend(down.components().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// A path from the working directory, written with `/`, as the type check writes it.
fn shown(base: &Path, path: &Path) -> String {
    match path.strip_prefix(base) {
        Ok(rest) => rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_is_named_after_its_file() {
        assert_eq!(module_name("get_user").as_deref(), Some("get_user"));
        assert_eq!(module_name("type").as_deref(), Some("r#type"));
        assert_eq!(module_name("get-user"), None);
        assert_eq!(module_name("1st"), None);
    }

    #[test]
    fn a_path_is_written_from_the_file_that_includes_it() {
        assert_eq!(relative(Path::new("/a/src"), Path::new("/a/src/queries/get.sql")), "queries/get.sql");
        assert_eq!(relative(Path::new("/a/src/out"), Path::new("/a/src/queries/get.sql")), "../queries/get.sql");
        assert_eq!(relative(Path::new("/a/src"), Path::new("/a/src")), ".");
    }

    #[test]
    fn the_common_directory_is_the_deepest_one_every_file_is_in() {
        let dirs = [Path::new("/a/src/q/users"), Path::new("/a/src/q/posts"), Path::new("/a/src/q")];
        assert_eq!(common_dir(dirs.into_iter()), PathBuf::from("/a/src/q"));
        assert_eq!(common_dir([Path::new("/a/src/q")].into_iter()), PathBuf::from("/a/src/q"));
    }

    #[test]
    fn a_diagnostic_is_placed_by_line_and_column() {
        let sql = "select id\nfrom userz";
        let d = serde_json::json!({ "message": "relation \"userz\" does not exist", "start": 15 });
        assert_eq!(diagnostic("q/get.sql", sql, &d), "q/get.sql:2:6: relation \"userz\" does not exist");
    }

    #[test]
    fn a_file_it_did_not_write_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("reylo-generate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("get_user.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        assert!(write(&path, &format!("{MARK} from get_user.sql\n")).unwrap_err().0.contains("reylo generate did not write it"));
        std::fs::remove_file(&path).unwrap();
        let text = format!("{MARK} from get_user.sql\n");
        assert_eq!(write(&path, &text).unwrap().changed, true);
        assert_eq!(write(&path, &text).unwrap().changed, false);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
