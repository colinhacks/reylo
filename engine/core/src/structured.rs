//! Structured parameters: a named parameter cast to `rows(…)` or `row(…)`, whose column list
//! names each key the caller passes, its database type, and whether it may be null or left out
//! (wiki/structured-params.md). The caller passes one
//! JSON value, an array of objects for `rows` and one object for `row`, and the statement reads it
//! as a table:
//!
//! ```sql
//! insert into events :events::rows(id int8 not null, status? text not null, meta jsonb('{…}'))
//! returning id
//! ```
//!
//! The pass runs before the analysis. It finds each such parameter in the author's text, reads
//! its column list, and writes in its place what the database reads instead, keeping the author's
//! placeholder, so the rest of the analysis sees an ordinary statement with one JSON parameter.
//! What it writes depends on where the parameter stands:
//! - right after `INTO <table>`, the insert's column list and a query over the rows, in the
//!   array's order (`wiki/bulk-insert.md`, Order);
//! - anywhere else, a table, named by the alias written after it or else by the parameter.
//!
//! Each dialect reads the JSON with its own reader: `jsonb_to_record` on Postgres, `json_each` on
//! SQLite, `JSON_TABLE` with `ERROR ON ERROR` on every column on MySQL (which otherwise turns a
//! value it cannot read into NULL without a warning; `wiki/structured-params/verify.md`).
//!
//! A key a row leaves out takes its column's default. On Postgres and SQLite the insert reads,
//! for each member whose key may be left out, the row's value where it has the key and else the
//! column's default copied from the catalog ([`copies`]), so one statement takes rows with any mix
//! of keys. On MySQL the run time writes the insert's lists again per call
//! without the keys it leaves out ([`Info`]), and refuses a call whose rows give different keys.
//!
//! The column list is written bare in the cast's parentheses. A member is `name[?] type [not null]`: `?` lets the caller leave a `not null` key out so that the column's default
//! applies, a member without `not null` may be null or left out, and `json('…')` or `jsonb('…')`
//! types a JSON column by the JSON Schema in its string. Strings in the column list are read the
//! standard way on every dialect, `''` for a quote and no backslash escapes, since the database
//! never reads them.
//!
//! The same pass writes a parameter's `::type` cast on MySQL and SQLite, which have no `::`, as the
//! dialect's own cast (`::` everywhere, beside each dialect's own cast syntax, converted by the
//! compiler). The types are written as on Postgres, and each dialect
//! takes its own spelling of one: MySQL's `CAST` takes `SIGNED` for an integer, and SQLite's
//! reads a type by its affinity, which for a date or a boolean is NUMERIC and turns `'2026-10-12'`
//! into 2026, so a date is cast to TEXT, as SQLite keeps one. MySQL refuses an expression in
//! `LIMIT` and `OFFSET`, a window frame's bound, NTILE's argument and the offset of LAG and LEAD,
//! which take only a literal or a bare `?`, so a cast there is the bare placeholder. A text cast
//! on MySQL (`::text`, `::varchar(n)`, `::char(n)`, `::uuid`) is the bare placeholder too, since
//! 2026-10-20: a CAST to CHAR takes the connection's collation, which a column of another
//! collation refuses to compare with ("Illegal mix of collations"), while a bare placeholder takes
//! the column's; the parameter is typed as text all the same, which MySQL does for a bare
//! placeholder and MariaDB does not. Both databases' casts turn a value they cannot read into 0 or
//! NULL instead of failing, so the run time checks such a parameter against its type before the
//! statement runs, a text cast's included.
//!
//! A parameter cast to `nullable(…)`, as in `:q::nullable(text)` or `:doc::nullable(jsonb('…'))`,
//! takes null, and the caller may leave it out: the pass writes the cast inside alone, which lets
//! a NULL through in every dialect, and hands the dialect's analysis the names of such parameters,
//! which it takes as the ones that may be null in place of any rule of its own.

use crate::api::{Analysis, Describe, Diagnostic, Scope};
use crate::dialect::Dialect;
use crate::host::{as_bool, as_str, Host, Lazy};
use crate::lex;
use crate::types::{RowField, Type};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One key of a column list.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub name: String,
    /// The type as written, with its spacing made regular: `int8`, `numeric(12,2)`,
    /// `timestamp with time zone`; `jsonb` or `json` for a member that carries a schema.
    pub ty: String,
    /// The JSON Schema of a `json('…')` or `jsonb('…')` member.
    pub schema: Option<Value>,
    pub not_null: bool,
    /// `?`: a `not null` key the caller may leave out, so that the column's default applies.
    pub optional: bool,
    /// Byte offsets in the author's text: the whole member, its name and its type.
    pub span: (usize, usize),
    pub name_span: (usize, usize),
    pub type_span: (usize, usize),
}

impl Member {
    /// Whether the caller may leave the key out: a `?` member, or one that may be null.
    pub fn omittable(&self) -> bool {
        self.optional || !self.not_null
    }
}

/// One structured parameter as the author wrote it.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The parameter's name, prefix dropped, as the lexer names it.
    pub name: String,
    pub placeholder: (usize, usize),
    /// `row` rather than `rows`.
    pub one: bool,
    pub members: Vec<Member>,
    /// From the placeholder to the descriptor's closing parenthesis, and past an alias the author
    /// wrote after it.
    pub region: (usize, usize),
    /// `:events::rows`, where a diagnostic about the parameter as a whole points.
    pub head: (usize, usize),
    /// The alias the author wrote after the parameter, as written (`e`, `"Events"`).
    pub alias: Option<String>,
    /// Right after `INTO <table>`.
    pub insert: bool,
    /// That table's name as written, in insert position.
    pub table: Option<(usize, usize)>,
    /// A typed JSON parameter (`:p::jsonb('…')`) instead of a column list.
    pub json: Option<JsonCast>,
    /// A `::type` cast on MySQL or SQLite instead of a column list, or on any dialect the type
    /// inside `::nullable(…)`.
    pub plain: Option<PlainCast>,
    /// Cast to `nullable(…)`: the caller may pass null, or leave the parameter out, and the
    /// parameter is its inner cast otherwise.
    pub nullable: bool,
    /// In insert position on Postgres and SQLite, per member, the default copied from the catalog
    /// that a row which leaves the member's key out takes, `None` for a member that takes none
    /// (required, or with no default, where the missing key's NULL is the column's own); `None`
    /// as a whole where the run time writes the lists again per call instead (MySQL)
    /// ([`copies`]).
    pub copied: Option<Vec<Option<String>>>,
}

/// A `::type` cast on a dialect that has no `::`: the type as written, and the text the expansion
/// writes before and after the placeholder, or none where it writes the bare placeholder (MySQL's
/// `LIMIT` and `OFFSET`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlainCast {
    pub written: String,
    pub around: Option<(String, String)>,
}

/// A typed JSON parameter, `:p::jsonb('…')` or `:p::json('…')` (wiki/typed-json-params.md): its
/// JSON Schema, and where the schema's string is written.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonCast {
    /// `jsonb` rather than `json`.
    pub jsonb: bool,
    pub schema: Value,
    pub schema_span: (usize, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

fn error<T>(message: impl Into<String>, start: usize, end: usize) -> Result<T, DescriptorError> {
    Err(DescriptorError { message: message.into(), start, end: end.max(start + 1) })
}

/// What the expansion writes differently for the MySQL-family server it is written for, read by
/// [`MYSQL_SERVER_QUERY`] in a round of its own, before the statement is expanded, since what is
/// described depends on it (docs/structured-params.md, Positions, 2026-10-20). Postgres and SQLite
/// need nothing, and a server that does not answer is taken as MySQL with no collation known.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Server {
    /// MariaDB, whose JSON is LONGTEXT: it has no `CAST(… AS JSON)`, keeps a JSON null as the
    /// text `null` in a JSON_TABLE column, takes no placeholder as a window frame's bound, and
    /// reports a JSON column as `longtext` with a `json_valid` CHECK.
    pub mariadb: bool,
    /// The database's default character set and collation, which a MySQL JSON_TABLE's text
    /// columns are given explicitly: such a column otherwise takes the connection's collation
    /// (`utf8mb4_unicode_ci` under mysql2), and a comparison with a table's column of another
    /// one fails with "Illegal mix of collations".
    pub collation: Option<(String, String)>,
}

/// The server's version, which names MariaDB, and the database's character set and collation.
pub const MYSQL_SERVER_QUERY: &str = "select version(), @@character_set_database, @@collation_database";

impl Server {
    /// The facts in the rows [`MYSQL_SERVER_QUERY`] answered with.
    pub fn of(rows: &[Vec<Value>]) -> Server {
        let Some(row) = rows.first() else { return Server::default() };
        let text = |k: usize| row.get(k).and_then(as_str).map(str::to_string);
        // A name the expansion writes bare must be a plain collation name.
        let plain = |s: &String| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        Server {
            mariadb: text(0).is_some_and(|v| v.to_ascii_lowercase().contains("mariadb")),
            collation: match (text(1), text(2)) {
                (Some(cs), Some(co)) if plain(&cs) && plain(&co) => Some((cs, co)),
                _ => None,
            },
        }
    }
}

/// Whether this pass rewrites the statement at all, without reading what it finds: a cheap test
/// for a caller that treats such statements apart. A structured parameter (`::rows(…)`,
/// `::row(…)`), a typed JSON parameter (`::jsonb('…')`), a cast to `nullable(…)` and, on MySQL
/// and SQLite, which have no `::`, any other `::type` are each written into the statement
/// differently than the lexer alone writes them.
pub fn present(sql: &str, dialect: Dialect) -> bool {
    let Ok(lexed) = lex::lex(sql, dialect) else { return false };
    lexed.placeholders.iter().any(|p| cast_at(sql, p.end).is_some() || json_cast_at(sql, p.end).is_some() || nullable_cast_at(sql, p.end).is_some() || (dialect != Dialect::Postgres && plain_cast_at(sql, p.end).is_some()))
}

/// After a placeholder ending at `end`: `::rows(` or `::row(`, with any spacing, as `(one, the
/// offset of the parenthesis, the end of the keyword)`.
fn cast_at(sql: &str, end: usize) -> Option<(bool, usize, usize)> {
    let b = sql.as_bytes();
    let mut i = skip_space(b, end);
    if !(b.get(i) == Some(&b':') && b.get(i + 1) == Some(&b':')) {
        return None;
    }
    i = skip_space(b, i + 2);
    let word_end = i + b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
    let word = sql[i..word_end].to_ascii_lowercase();
    let one = match word.as_str() {
        "rows" => false,
        "row" => true,
        _ => return None,
    };
    let paren = skip_space(b, word_end);
    (b.get(paren) == Some(&b'(')).then_some((one, paren, word_end))
}

/// After a placeholder ending at `end`: `::jsonb(` or `::json(`, as `(jsonb, the offset of the
/// parenthesis, the end of the keyword)`. Without the parenthesis it is an ordinary cast.
fn json_cast_at(sql: &str, end: usize) -> Option<(bool, usize, usize)> {
    json_type_at(sql, after_colons(sql, end)?)
}

/// After a placeholder ending at `end`: `::nullable(`, as `(the offset of the parenthesis, the end
/// of the keyword)`.
fn nullable_cast_at(sql: &str, end: usize) -> Option<(usize, usize)> {
    let b = sql.as_bytes();
    let i = after_colons(sql, end)?;
    let word_end = i + b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
    if !sql[i..word_end].eq_ignore_ascii_case("nullable") {
        return None;
    }
    let paren = skip_space(b, word_end);
    (b.get(paren) == Some(&b'(')).then_some((paren, word_end))
}

/// Where the type of a `::` cast after a placeholder ending at `end` starts.
fn after_colons(sql: &str, end: usize) -> Option<usize> {
    let b = sql.as_bytes();
    let i = skip_space(b, end);
    (b.get(i) == Some(&b':') && b.get(i + 1) == Some(&b':')).then(|| skip_space(b, i + 2))
}

/// At `i`: `jsonb(` or `json(`, as `(jsonb, the offset of the parenthesis, the end of the keyword)`.
fn json_type_at(sql: &str, i: usize) -> Option<(bool, usize, usize)> {
    let b = sql.as_bytes();
    let word_end = i + b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
    let jsonb = match sql[i..word_end].to_ascii_lowercase().as_str() {
        "jsonb" => true,
        "json" => false,
        _ => return None,
    };
    let paren = skip_space(b, word_end);
    (b.get(paren) == Some(&b'(')).then_some((jsonb, paren, word_end))
}

/// After a placeholder ending at `end`: `::type`, as `(the type as written, lowercased and spaced
/// once, the end of the cast)` ([`plain_type_at`]).
fn plain_cast_at(sql: &str, end: usize) -> Option<(String, usize)> {
    plain_type_at(sql, after_colons(sql, end)?)
}

/// At `i`: a type, as `(the type as written, lowercased and spaced once, its end)`. A type is a
/// name, a second word where SQL spells one type in two (`double precision`, `character varying`,
/// `unsigned`), a parenthesized modifier list, and `[]`.
fn plain_type_at(sql: &str, i: usize) -> Option<(String, usize)> {
    let b = sql.as_bytes();
    let word = |i: usize| i + b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
    let first = word(i);
    if first == i {
        return None;
    }
    let mut written = sql[i..first].to_ascii_lowercase();
    let mut j = first;
    let after = skip_space(b, j);
    let second = word(after);
    if second > after && matches!((written.as_str(), sql[after..second].to_ascii_lowercase().as_str()), ("double", "precision") | ("character", "varying") | (_, "unsigned")) {
        written = format!("{written} {}", sql[after..second].to_ascii_lowercase());
        j = second;
    }
    let paren = skip_space(b, j);
    if b.get(paren) == Some(&b'(') {
        let close = paren + b[paren..].iter().position(|&c| c == b')')?;
        if sql[paren + 1..close].chars().all(|c| c.is_ascii_digit() || c == ',' || c.is_whitespace()) {
            written.push_str(&format!("({})", sql[paren + 1..close].chars().filter(|c| !c.is_whitespace()).collect::<String>()));
            j = close + 1;
        }
    }
    if sql[j..].starts_with("[]") {
        written.push_str("[]");
        j += 2;
    }
    Some((written, j))
}

/// Where a cast parameter stands, for the places MySQL takes only a literal or a bare `?`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spot {
    /// Anywhere an expression is taken.
    Expression,
    /// A `LIMIT` or `OFFSET` value.
    Limit,
    /// A window frame's bound, `ROWS BETWEEN ? PRECEDING`.
    Frame,
    /// `NTILE(?)`, or the offset of `LAG(x, ?)` and `LEAD(x, ?)`.
    WindowArgument,
}

/// Where the placeholder from `start` to `end`, followed by its cast ending at `cast_end`, stands.
fn spot(sql: &str, start: usize, cast_end: usize, opaque: &[(usize, usize)]) -> Spot {
    if in_limit(sql, start) {
        return Spot::Limit;
    }
    let b = sql.as_bytes();
    let after = skip_space(b, cast_end);
    let word = &sql[after..after + b[after..].iter().take_while(|c| c.is_ascii_alphabetic()).count()];
    if word.eq_ignore_ascii_case("preceding") || word.eq_ignore_ascii_case("following") {
        return Spot::Frame;
    }
    // Back past spaces to `(` (NTILE's one argument) or to `,` after LAG's or LEAD's first one.
    let mut i = start;
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    let function_before = |open: usize| {
        let mut e = open;
        while e > 0 && b[e - 1].is_ascii_whitespace() {
            e -= 1;
        }
        let mut s = e;
        while s > 0 && (b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_') {
            s -= 1;
        }
        sql[s..e].to_ascii_lowercase()
    };
    if i > 0 && b[i - 1] == b'(' {
        return if function_before(i - 1) == "ntile" { Spot::WindowArgument } else { Spot::Expression };
    }
    if i > 0 && b[i - 1] == b',' {
        // The first argument: back to the parenthesis that opens the call, past nested ones and
        // strings, with no other comma at its depth.
        let mut depth = 0usize;
        let mut k = i - 1;
        while k > 0 {
            let c = b[k - 1];
            if let Some(&(s, _)) = opaque.iter().find(|&&(_, e)| e == k) {
                k = s;
                continue;
            }
            match c {
                b')' => depth += 1,
                b'(' if depth == 0 => {
                    let f = function_before(k - 1);
                    return if f == "lag" || f == "lead" { Spot::WindowArgument } else { Spot::Expression };
                }
                b'(' => depth -= 1,
                b',' if depth == 0 => return Spot::Expression,
                _ => {}
            }
            k -= 1;
        }
    }
    Spot::Expression
}

/// Whether the placeholder at `start` is a MySQL `LIMIT` or `OFFSET` value, `LIMIT a, b`'s
/// first included.
fn in_limit(sql: &str, start: usize) -> bool {
    let b = sql.as_bytes();
    let back = |mut i: usize| {
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        i
    };
    let word_before = |end: usize| {
        let mut s = end;
        while s > 0 && (b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_') {
            s -= 1;
        }
        sql[s..end].to_ascii_lowercase()
    };
    let mut i = back(start);
    if i > 0 && b[i - 1] == b',' {
        // `LIMIT a, b`: past the comma, the first value and the keyword before it.
        i = back(i - 1);
        while i > 0 && !b[i - 1].is_ascii_whitespace() && b[i - 1] != b',' {
            i -= 1;
        }
        return word_before(back(i)) == "limit";
    }
    matches!(word_before(i).as_str(), "limit" | "offset")
}

/// Whether a type as written is an integer: `int`, `bigint unsigned`, `signed`.
fn integer_type(written: &str) -> bool {
    let base = written.split('(').next().unwrap_or(written).trim_end_matches(" unsigned");
    matches!(base, "int" | "int2" | "int4" | "int8" | "integer" | "bigint" | "smallint" | "tinyint" | "mediumint" | "signed" | "unsigned")
}

/// Whether a type as written, without its modifiers, is one MySQL keeps as text: a string type,
/// or a UUID, which MySQL keeps as CHAR(36).
fn mysql_text(base: &str) -> bool {
    matches!(base, "text" | "varchar" | "char" | "character" | "character varying" | "bpchar" | "uuid")
}

/// What a `::type` cast after a parameter is written as on MySQL or SQLite: the text before and
/// after the placeholder, none for the bare placeholder, or why the dialect cannot cast to it.
fn cast_around(written: &str, dialect: Dialect, spot: Spot, server: &Server) -> Result<Option<(String, String)>, String> {
    let db = if dialect == Dialect::Sqlite { "SQLite" } else if server.mariadb { "MariaDB" } else { "MySQL" };
    if written.ends_with("[]") {
        return Err(format!("{db} has no arrays: `::{written}` cannot be written as its cast."));
    }
    // Where MySQL takes only a literal or a bare `?`, the cast is the bare placeholder, and the
    // run time checks the value against the type written.
    if dialect == Dialect::Mysql {
        let numeric = |t: &str| integer_type(t) || matches!(t.split('(').next().unwrap_or(t), "numeric" | "decimal" | "dec" | "real" | "float" | "float4" | "float8" | "double" | "double precision");
        match spot {
            Spot::Limit => return if integer_type(written) { Ok(None) } else { Err(format!("`LIMIT` and `OFFSET` take an integer, as in `::int`, not `::{written}`.")) },
            Spot::WindowArgument => return if integer_type(written) { Ok(None) } else { Err(format!("NTILE's argument and the offset of LAG and LEAD take an integer, as in `::int`, not `::{written}`.")) },
            Spot::Frame if server.mariadb => return Err("MariaDB takes only a literal as a window frame's bound, never a parameter: write the number in the frame.".to_string()),
            Spot::Frame => return if numeric(written) { Ok(None) } else { Err(format!("A window frame's bound takes a number, as in `::int`, not `::{written}`.")) },
            Spot::Expression => {}
        }
    }
    let cast = |t: &str| Ok(Some(("CAST(".to_string(), format!(" AS {t})"))));
    let (base, modifiers) = match written.find('(') {
        Some(k) => (&written[..k], &written[k..]),
        None => (written, ""),
    };
    if dialect == Dialect::Sqlite {
        return match base {
            // SQLite's own conversions of JSON text, which fail on text that is not JSON; a value
            // passed as text alone reads as a string inside SQLite's JSON functions.
            "json" | "jsonb" => Ok(Some((format!("{base}("), ")".to_string()))),
            "bool" | "boolean" => Err(format!("SQLite has no boolean type, and its CAST to `{base}` reads a value as the number it starts with: pass 1 or 0 and write `::integer`.")),
            // SQLite keeps dates, times and UUIDs as text; its CAST to `date` reads `'2026-10-12'`
            // as 2026.
            "date" | "time" | "timestamp" | "timestamptz" | "datetime" | "uuid" => cast("TEXT"),
            "numeric" | "decimal" => cast("NUMERIC"),
            _ => match affinity(written) {
                "INTEGER" => cast("INTEGER"),
                "TEXT" => cast("TEXT"),
                "REAL" => cast("REAL"),
                "BLOB" if base == "blob" => cast("BLOB"),
                _ => Err(format!("SQLite has no type `{written}`, and its CAST would read the value as a number: write `::integer`, `::real`, `::numeric`, `::text` or `::blob`.")),
            },
        };
    }
    // MySQL's CAST takes neither `int` nor `text`.
    match base {
        b if integer_type(b) && b.ends_with("unsigned") => cast("UNSIGNED"),
        b if integer_type(b) => cast("SIGNED"),
        "numeric" | "decimal" | "dec" | "fixed" => cast(&format!("DECIMAL{modifiers}")),
        "float4" | "real" | "float" => cast("FLOAT"),
        "float8" | "double" | "double precision" => cast("DOUBLE"),
        // A CAST to CHAR takes the connection's collation with IMPLICIT coercibility
        // (`utf8mb4_unicode_ci` under mysql2), which a column of another one refuses to compare
        // with ("Illegal mix of collations"), while a bare placeholder is COERCIBLE and takes the
        // column's. So the cast is the bare placeholder, typed as text (`analyze` below),
        // and the run time still checks the value is a string that fits.
        b if mysql_text(b) => Ok(None),
        "date" => cast("DATE"),
        "time" => cast(&format!("TIME{modifiers}")),
        "timestamp" | "datetime" => cast(&if modifiers.is_empty() { "DATETIME(6)".to_string() } else { format!("DATETIME{modifiers}") }),
        // MariaDB has no `CAST(… AS JSON)`; JSON_COMPACT() reads the text as JSON.
        "json" | "jsonb" if server.mariadb => Ok(Some(("JSON_COMPACT(".to_string(), ")".to_string()))),
        "json" | "jsonb" => cast("JSON"),
        "bytea" | "binary" | "varbinary" | "blob" => cast(&format!("BINARY{modifiers}")),
        "timestamptz" => Err("MySQL has no type for an instant with its zone: DATETIME keeps the wall-clock time and drops the zone. Write `::datetime` for a wall-clock time.".to_string()),
        "bool" | "boolean" => Err(format!("MySQL has no boolean type to cast to: `::{written}` would make the value a number. Compare the parameter instead, or pass 1 or 0 and write `::int`.")),
        _ => Err(format!("MySQL's CAST has no type for `::{written}`: write MySQL's own `CAST(… AS …)`, whose types are SIGNED, UNSIGNED, DECIMAL, FLOAT, DOUBLE, CHAR, DATE, TIME, DATETIME, YEAR, JSON and BINARY.")),
    }
}

fn skip_space(b: &[u8], mut i: usize) -> usize {
    while b.get(i).is_some_and(|c| c.is_ascii_whitespace()) {
        i += 1;
    }
    i
}

/// Every structured parameter of the statement, in text order, or the first error in one's
/// column list. A statement the lexer cannot read has none: the analysis reports why. `server`
/// matters on MySQL alone ([`Server`]).
pub fn find(sql: &str, dialect: Dialect, server: &Server) -> Result<Vec<Found>, DescriptorError> {
    let Ok(lexed) = lex::lex(sql, dialect) else { return Ok(Vec::new()) };
    let mut found: Vec<Found> = Vec::new();
    for p in &lexed.placeholders {
        // A placeholder the lexer found inside an earlier column list (`status?` reads as a `?`
        // on MySQL and SQLite) is part of it.
        if found.last().is_some_and(|f| p.start < f.region.1) {
            continue;
        }
        if let Some((paren, _)) = nullable_cast_at(sql, p.end) {
            found.push(nullable_found(sql, p, paren, dialect, &lexed.opaque, server)?);
            continue;
        }
        if let Some((jsonb, paren, _)) = json_cast_at(sql, p.end) {
            let (cast, close) = json_cast(sql, &sql[p.start..p.end], jsonb, paren)?;
            found.push(Found { name: p.name.clone(), placeholder: (p.start, p.end), one: false, members: Vec::new(), region: (p.start, close + 1), head: (p.start, p.end), alias: None, insert: false, table: None, json: Some(cast), plain: None, nullable: false, copied: None });
            continue;
        }
        let Some((one, paren, word_end)) = cast_at(sql, p.end) else {
            // Any other `::type` on a dialect that has no `::`.
            if dialect != Dialect::Postgres {
                if let Some((written, end)) = plain_cast_at(sql, p.end) {
                    let around = cast_around(&written, dialect, spot(sql, p.start, end, &lexed.opaque), server).map_err(|message| DescriptorError { message, start: p.end, end })?;
                    found.push(Found { name: p.name.clone(), placeholder: (p.start, p.end), one: false, members: Vec::new(), region: (p.start, end), head: (p.start, end), alias: None, insert: false, table: None, json: None, plain: Some(PlainCast { written, around }), nullable: false, copied: None });
                }
            }
            continue;
        };
        if lex::is_positional(&p.name) {
            return error("A rows or row parameter is named: write :name::rows(…), not a positional placeholder.", p.start, word_end);
        }
        let (content, close) = descriptor_text(sql, paren)?;
        let members = members(&content, (paren, close + 1))?;
        let mut region = (p.start, close + 1);
        // `INTO <table>`, or on Postgres and SQLite `INTO <table> AS <alias>`.
        let table = after_into(sql, p.start, &lexed.opaque).or_else(|| if dialect == Dialect::Mysql { None } else { after_into_aliased(sql, p.start, &lexed.opaque) });
        let insert = table.is_some();
        if !insert {
            if let Some(list) = own_column_list(sql, p.start, &lexed.opaque) {
                let (s, e) = list;
                return error(
                    format!("A {} cast right after INSERT INTO <table> writes the insert's column list itself, one column per member: drop `{}` and write the parameter right after the table.", if one { "row" } else { "rows" }, &sql[s..e]),
                    s,
                    e,
                );
            }
        }
        let mut alias = None;
        if !insert {
            if let Some((name, end)) = alias_after(sql, close + 1) {
                alias = Some(name);
                region.1 = end;
            }
        } else if dialect == Dialect::Mysql {
            // MySQL's row alias, `… AS new ON DUPLICATE KEY UPDATE x = new.x`, which MySQL takes
            // only after VALUES: here it names the JSON_TABLE the rows are read from.
            let b = sql.as_bytes();
            let i = skip_space(b, close + 1);
            let w = i + b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
            if w > i && sql[i..w].eq_ignore_ascii_case("as") {
                if let Some((name, end)) = alias_after(sql, close + 1) {
                    alias = Some(name);
                    region.1 = end;
                }
            }
        }
        if !insert {
            if let Some(m) = members.iter().find(|m| m.optional) {
                return error(
                    format!("`{}?` lets the caller leave a key out so that the column's default applies, which happens only where the rows are inserted, right after INSERT INTO <table>; here a missing key reads as NULL, so write the member without `?` and without `not null`.", m.name),
                    m.name_span.0,
                    m.name_span.1 + 1,
                );
            }
        }
        found.push(Found { name: p.name.clone(), placeholder: (p.start, p.end), one, members, region, head: (p.start, word_end), alias, insert, table, json: None, plain: None, nullable: false, copied: None });
    }
    Ok(found)
}

/// A typed JSON parameter's cast after `placeholder`, `json(` or `jsonb(` opening at `paren`, and
/// the offset of its closing parenthesis.
fn json_cast(sql: &str, placeholder: &str, jsonb: bool, paren: usize) -> Result<(JsonCast, usize), DescriptorError> {
    let word = if jsonb { "jsonb" } else { "json" };
    let start = skip_space(sql.as_bytes(), paren + 1);
    if sql.as_bytes().get(start) != Some(&b'\'') {
        let close = descriptor_text(sql, paren).map(|(_, close)| close).unwrap_or(sql.len() - 1);
        return error(format!("`{word}(…)` takes one string, the JSON Schema of the values, as in `{placeholder}::{word}('{{\"type\": \"object\"}}')`."), paren, close + 1);
    }
    let (text, close) = schema_string(sql, start)?;
    let span = (start, close);
    let schema = match serde_json::from_str::<Value>(&text) {
        Ok(schema) => schema,
        Err(e) => return error(format!("The schema in the cast of {placeholder} is not JSON: {e}."), span.0, span.1),
    };
    if let Some(message) = profile(&schema, "") {
        return error(message, span.0, span.1);
    }
    Ok((JsonCast { jsonb, schema, schema_span: span }, close))
}

/// A parameter cast to `nullable(…)`, whose parenthesis opens at `paren`: the caller may pass null
/// or leave it out, and it is the cast inside otherwise, a typed JSON one or a type, which the
/// expansion writes alone. Postgres reads that type as written (a schema's, a quoted name,
/// `timestamp with time zone`); MySQL and SQLite take it as their own cast, as they take `::type`.
fn nullable_found(sql: &str, p: &lex::Placeholder, paren: usize, dialect: Dialect, opaque: &[(usize, usize)], server: &Server) -> Result<Found, DescriptorError> {
    let b = sql.as_bytes();
    let placeholder = &sql[p.start..p.end];
    let takes = format!("`nullable(…)` takes the parameter's type, as in `{placeholder}::nullable(text)`");
    let start = skip_space(b, paren + 1);
    if b.get(start) == Some(&b'\'') {
        return error(format!("{takes}, not a string."), start, start + 1);
    }
    let (_, close) = descriptor_text(sql, paren)?;
    let end = sql[..close].trim_end().len().max(start);
    let mut f = Found { name: p.name.clone(), placeholder: (p.start, p.end), one: false, members: Vec::new(), region: (p.start, close + 1), head: (p.start, close + 1), alias: None, insert: false, table: None, json: None, plain: None, nullable: true, copied: None };
    if start == end {
        return error(format!("{takes}."), paren, close + 1);
    }
    if let Some((jsonb, json_paren, _)) = json_type_at(sql, start) {
        let (cast, json_close) = json_cast(sql, placeholder, jsonb, json_paren)?;
        if skip_space(b, json_close + 1) != close {
            return error(format!("{takes}, one type alone."), json_close + 1, close);
        }
        f.json = Some(cast);
        return Ok(f);
    }
    let word_end = start + b[start..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
    match sql[start..word_end].to_ascii_lowercase().as_str() {
        "rows" | "row" if b.get(skip_space(b, word_end)) == Some(&b'(') => {
            return error(format!("A row(…) or rows(…) parameter is never null: {takes}."), start, end);
        }
        "nullable" => return error(format!("{takes} once."), start, end),
        _ => {}
    }
    if dialect == Dialect::Postgres {
        let written = &sql[start..end];
        if !one_postgres_type(written) {
            return error(format!("{takes}, one type alone."), start, end);
        }
        f.plain = Some(PlainCast { written: written.to_string(), around: Some((String::new(), format!("::{written}"))) });
        return Ok(f);
    }
    let Some((written, type_end)) = plain_type_at(sql, start).filter(|&(_, type_end)| skip_space(b, type_end) == close) else {
        return error(format!("{takes}, one type alone."), start, end);
    };
    let around = cast_around(&written, dialect, spot(sql, p.start, close + 1, opaque), server).map_err(|message| DescriptorError { message, start, end: type_end })?;
    f.plain = Some(PlainCast { written, around });
    Ok(f)
}

/// Whether `written`, the text inside a Postgres `nullable(…)`, reads as one type name, which the
/// expansion writes after `::` as it stands: a name, qualified or quoted, its modifiers in
/// parentheses, array bounds, and the words of Postgres's own types of several words (`double
/// precision`, `timestamp with time zone`, `interval day to second`). Anything else, a comma, an
/// operator, a string or another word, would turn the cast into an expression the server takes,
/// and change what the statement means. A modifier's text is the server's to check, which takes
/// only constants there.
fn one_postgres_type(written: &str) -> bool {
    const WORDS: &[&str] = &["precision", "varying", "with", "without", "time", "zone", "character", "char", "national", "year", "month", "day", "hour", "minute", "second", "to", "array"];
    #[derive(PartialEq)]
    enum Last {
        Start,
        Part,
        Dot,
        Space,
        Group,
    }
    let b = written.as_bytes();
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut last = Last::Start;
    let mut i = 0;
    while i < b.len() {
        let c = written[i..].chars().next().expect("inside the text");
        if c.is_whitespace() {
            if last != Last::Dot {
                last = if last == Last::Start { Last::Start } else { Last::Space };
            }
            i += c.len_utf8();
        } else if c == '"' || word(c) {
            let begin = i;
            if c == '"' {
                i += 1;
                loop {
                    match b.get(i) {
                        None => return false,
                        Some(b'"') if b.get(i + 1) == Some(&b'"') => i += 2,
                        Some(b'"') => break,
                        Some(_) => i += 1,
                    }
                }
                i += 1;
            } else {
                i += written[i..].chars().take_while(|c| word(*c)).map(char::len_utf8).sum::<usize>();
            }
            // A part after the first name is one of the words of Postgres's own types.
            match last {
                Last::Start | Last::Dot => {}
                Last::Space | Last::Group if c != '"' && WORDS.contains(&written[begin..i].to_ascii_lowercase().as_str()) => {}
                _ => return false,
            }
            last = Last::Part;
        } else if c == '.' {
            if last != Last::Part {
                return false;
            }
            last = Last::Dot;
            i += 1;
        } else if c == '(' || c == '[' {
            if matches!(last, Last::Start | Last::Dot) {
                return false;
            }
            let close = if c == '(' { ')' } else { ']' };
            let mut depth = 0usize;
            let mut quote: Option<u8> = None;
            loop {
                let Some(&d) = b.get(i) else { return false };
                match quote {
                    Some(q) if d == q => quote = None,
                    Some(_) => {}
                    None if d == b'\'' || d == b'"' => quote = Some(d),
                    None if d == c as u8 => depth += 1,
                    None if d == close as u8 => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    // Array bounds are digits.
                    None if c == '[' && !(d.is_ascii_digit() || d.is_ascii_whitespace()) => return false,
                    None => {}
                }
                i += 1;
            }
            i += 1;
            last = Last::Group;
        } else {
            return false;
        }
    }
    matches!(last, Last::Part | Last::Group | Last::Space)
}

/// One character of a column list's text and the bytes of the author's text it stands for.
type Text = Vec<(char, usize, usize)>;

/// A typed JSON parameter's schema: the string opening at `start`, read the standard way (`''`
/// for a quote), and the offset of the parenthesis that must follow it.
fn schema_string(sql: &str, start: usize) -> Result<(String, usize), DescriptorError> {
    let b = sql.as_bytes();
    let mut text = String::new();
    let mut i = start + 1;
    loop {
        let Some(c) = sql[i..].chars().next() else { return error("The schema's string has no closing quote.", start, sql.len()) };
        if c == '\'' {
            if b.get(i + 1) == Some(&b'\'') {
                text.push('\'');
                i += 2;
                continue;
            }
            break;
        }
        text.push(c);
        i += c.len_utf8();
    }
    let close = skip_space(b, i + 1);
    if b.get(close) != Some(&b')') {
        return error("The schema's string is the only thing in the parentheses, as in `::jsonb('{\"type\": \"object\"}')`.", i + 1, close + 1);
    }
    Ok((text, close))
}

/// The column list from the parenthesis at `paren`: its characters with their offsets, and the
/// offset of the closing parenthesis.
fn descriptor_text(sql: &str, paren: usize) -> Result<(Text, usize), DescriptorError> {
    let b = sql.as_bytes();
    let start = skip_space(b, paren + 1);
    // The column list is written bare in the parentheses; a string there is the form the syntax
    // does not take.
    if b.get(start) == Some(&b'\'') {
        return error("A column list is written bare in the parentheses, as in `::rows(id int8 not null, name text)`, not as a string.", start, start + 1);
    }
    // The parentheses' content, up to the one that closes them, past strings and quoted names.
    let mut depth = 0usize;
    let mut i = paren;
    let mut text = Text::new();
    while i < sql.len() {
        let c = sql[i..].chars().next().expect("inside the text");
        match c {
            '(' => {
                depth += 1;
                if depth > 1 {
                    text.push((c, i, i + 1));
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((text, i));
                }
                text.push((c, i, i + 1));
            }
            '\'' | '"' | '`' => {
                let mut j = i + 1;
                text.push((c, i, j));
                loop {
                    let Some(d) = sql[j..].chars().next() else { return error("A string or quoted name in the column list has no closing quote.", i, sql.len()) };
                    text.push((d, j, j + d.len_utf8()));
                    j += d.len_utf8();
                    if d == c {
                        if sql[j..].starts_with(c) {
                            text.push((c, j, j + 1));
                            j += 1;
                            continue;
                        }
                        break;
                    }
                }
                i = j;
                continue;
            }
            _ => text.push((c, i, i + c.len_utf8())),
        }
        i += c.len_utf8();
    }
    error("The column list has no closing parenthesis.", paren, sql.len())
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// A bare word, as written.
    Word(String),
    /// A quoted name, unquoted.
    Name(String),
    /// A string literal, unquoted.
    Str(String),
    Punct(char),
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

fn tokens(text: &Text, whole: (usize, usize)) -> Result<Vec<Token>, DescriptorError> {
    let at = |k: usize| text.get(k).map(|&(_, s, _)| s).unwrap_or(whole.1);
    let end_of = |k: usize| text.get(k).map(|&(_, _, e)| e).unwrap_or(whole.1);
    let mut out = Vec::new();
    let mut k = 0;
    while k < text.len() {
        let (c, _, _) = text[k];
        if c.is_whitespace() {
            k += 1;
            continue;
        }
        let start = at(k);
        if c.is_alphanumeric() || c == '_' || c == '$' {
            let mut word = String::new();
            while k < text.len() && (text[k].0.is_alphanumeric() || text[k].0 == '_' || text[k].0 == '$') {
                word.push(text[k].0);
                k += 1;
            }
            out.push(Token { tok: Tok::Word(word), start, end: end_of(k - 1) });
            continue;
        }
        if c == '\'' || c == '"' || c == '`' {
            let mut value = String::new();
            k += 1;
            loop {
                let Some(&(d, _, _)) = text.get(k) else { return error("A string or quoted name in the column list has no closing quote.", start, whole.1) };
                k += 1;
                if d == c {
                    if text.get(k).is_some_and(|&(e, _, _)| e == c) {
                        value.push(c);
                        k += 1;
                        continue;
                    }
                    break;
                }
                value.push(d);
            }
            let tok = if c == '\'' { Tok::Str(value) } else { Tok::Name(value) };
            out.push(Token { tok, start, end: end_of(k - 1) });
            continue;
        }
        out.push(Token { tok: Tok::Punct(c), start, end: end_of(k) });
        k += 1;
    }
    Ok(out)
}

/// The words a column definition may hold that a column list does not: a descriptor says what
/// each key holds, and what the table does with it stays in the table.
const TABLE_WORDS: &[&str] = &["default", "primary", "unique", "check", "references", "generated", "constraint", "collate", "identity", "auto_increment", "autoincrement", "key"];

fn members(text: &Text, whole: (usize, usize)) -> Result<Vec<Member>, DescriptorError> {
    let toks = tokens(text, whole)?;
    // Split at the commas outside parentheses and brackets.
    let mut groups: Vec<Vec<Token>> = vec![Vec::new()];
    let mut depth = 0i32;
    for t in toks {
        match t.tok {
            Tok::Punct('(') | Tok::Punct('[') => depth += 1,
            Tok::Punct(')') | Tok::Punct(']') => depth -= 1,
            Tok::Punct(',') if depth == 0 => {
                groups.push(Vec::new());
                continue;
            }
            _ => {}
        }
        groups.last_mut().expect("one group at least").push(t);
    }
    if groups.len() == 1 && groups[0].is_empty() {
        return error("The column list is empty: name each key the caller passes, as in `rows(id int8 not null, name text)`.", whole.0, whole.1);
    }
    let mut out: Vec<Member> = Vec::new();
    for group in groups {
        let Some(first) = group.first() else {
            return error("The column list has an empty member: two commas in a row, or a comma at its end.", whole.0, whole.1);
        };
        let span = (first.start, group.last().expect("not empty").end);
        let (name, name_span) = match &first.tok {
            Tok::Word(w) | Tok::Name(w) => (w.clone(), (first.start, first.end)),
            _ => return error("A member starts with the key's name, as in `id int8 not null`.", first.start, first.end),
        };
        let mut rest = &group[1..];
        let optional = matches!(rest.first(), Some(Token { tok: Tok::Punct('?'), .. }));
        if optional {
            rest = &rest[1..];
        }
        // `not null` or `null` at the end.
        let lower = |t: &Token| match &t.tok {
            Tok::Word(w) => w.to_ascii_lowercase(),
            _ => String::new(),
        };
        let mut not_null = false;
        let n = rest.len();
        if n >= 2 && lower(&rest[n - 2]) == "not" && lower(&rest[n - 1]) == "null" {
            not_null = true;
            rest = &rest[..n - 2];
        } else if n >= 1 && lower(&rest[n - 1]) == "null" {
            rest = &rest[..n - 1];
        }
        let Some(type_first) = rest.first() else {
            return error(format!("`{name}` has no type: write one, as in `{name} text not null`."), span.0, span.1);
        };
        let type_span = (type_first.start, rest.last().expect("not empty").end);
        if let Some(t) = rest.iter().find(|t| TABLE_WORDS.contains(&lower(t).as_str())).or_else(|| rest.iter().find(|t| lower(t) == "not")) {
            let word = lower(t);
            let message = if word == "default" {
                format!("A column list never restates a default: leave `default` to the table, and write `{name}? <type> not null` to let the caller leave the key out so that the default applies.")
            } else if word == "not" {
                format!("`{name}` has `not` where only `not null` at the end of the member may stand.")
            } else {
                format!("A column list says what each key holds: its type and `not null`. `{word}` belongs to the table's definition.")
            };
            return error(message, t.start, t.end);
        }
        let head = lower(type_first);
        if (head == "rows" || head == "row") && matches!(rest.get(1), Some(Token { tok: Tok::Punct('('), .. })) {
            return error("A nested row or rows member is not built yet: nested writes wait until Reylo takes multi-statement or Postgres-only forms.", type_span.0, type_span.1);
        }
        let (ty, schema) = if (head == "json" || head == "jsonb") && rest.len() > 1 {
            match (&rest.get(1).map(|t| &t.tok), rest.get(2).map(|t| &t.tok), rest.get(3).map(|t| &t.tok), rest.len()) {
                (Some(Tok::Punct('(')), Some(Tok::Str(text)), Some(Tok::Punct(')')), 4) => match serde_json::from_str::<Value>(text) {
                    Ok(schema) if schema.get("default").is_some() => {
                        return error(format!("A key the row leaves out takes its column's default, by omission: `{name}`'s schema cannot give it one. Write `{name}?` with `not null` for a column that has a default."), rest[2].start, rest[2].end)
                    }
                    Ok(schema) if schema.is_object() || schema.is_boolean() => match profile(&schema, "") {
                        Some(message) => return error(message, rest[2].start, rest[2].end),
                        None => (head.clone(), Some(schema)),
                    },
                    Ok(_) => return error("A JSON member's schema is a JSON Schema: an object, as in `json('{\"type\": \"object\"}')`.", rest[2].start, rest[2].end),
                    Err(e) => return error(format!("The schema in `{name}`'s `{head}(…)` is not JSON: {e}."), rest[2].start, rest[2].end),
                },
                _ => return error(format!("`{head}(…)` takes one string, the JSON Schema of the values, as in `{name} {head}('{{\"type\": \"object\"}}')`."), type_span.0, type_span.1),
            }
        } else {
            (type_text(rest), None)
        };
        if optional && !not_null {
            return error(
                format!("`{name}?` marks a `not null` key the caller may leave out; `{name}` may be null, so it may be left out already. Write `{name}? {ty} not null`, or drop the `?`."),
                name_span.0,
                name_span.1 + 1,
            );
        }
        if out.iter().any(|m| m.name == name) {
            return error(format!("The column list names `{name}` twice."), name_span.0, name_span.1);
        }
        out.push(Member { name, ty, schema, not_null, optional, span, name_span, type_span });
    }
    Ok(out)
}

/// A type's tokens as one string, spaced as SQL writes it: `numeric(12,2)`, `text[]`,
/// `timestamp with time zone`.
fn type_text(toks: &[Token]) -> String {
    let mut out = String::new();
    let mut prev: Option<&Tok> = None;
    for t in toks {
        let piece = match &t.tok {
            Tok::Word(w) => w.clone(),
            Tok::Name(n) => format!("\"{}\"", n.replace('"', "\"\"")),
            Tok::Str(s) => format!("'{}'", s.replace('\'', "''")),
            Tok::Punct(c) => c.to_string(),
        };
        let tight = matches!(t.tok, Tok::Punct('(') | Tok::Punct(')') | Tok::Punct('[') | Tok::Punct(']') | Tok::Punct(',') | Tok::Punct('.'))
            || matches!(prev, Some(Tok::Punct('(')) | Some(Tok::Punct('[')) | Some(Tok::Punct(',')) | Some(Tok::Punct('.')) | None);
        if !tight {
            out.push(' ');
        }
        out.push_str(&piece);
        prev = Some(&t.tok);
    }
    out
}

/// The table's name when the text before `start`, past spaces, is `INTO <table>`, a table name
/// being a name or a quoted name, schema-qualified or not.
fn after_into(sql: &str, start: usize, opaque: &[(usize, usize)]) -> Option<(usize, usize)> {
    let b = sql.as_bytes();
    let mut i = start;
    let back_space = |mut i: usize| {
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        i
    };
    // The table name: names joined by dots, each bare or quoted.
    i = back_space(i);
    let name_end = i;
    loop {
        let end = i;
        if i > 0 && (b[i - 1] == b'"' || b[i - 1] == b'`') {
            let &(s, _) = opaque.iter().find(|&&(_, e)| e == end)?;
            i = s;
        } else {
            while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$') {
                i -= 1;
            }
            if i == end {
                return None;
            }
        }
        if i > 0 && b[i - 1] == b'.' {
            i -= 1;
            continue;
        }
        break;
    }
    let name_start = i;
    let end = back_space(i);
    let into = end >= 4 && sql[end - 4..end].eq_ignore_ascii_case("into") && (end == 4 || !(b[end - 5].is_ascii_alphanumeric() || b[end - 5] == b'_'));
    into.then_some((name_start, name_end))
}

/// The table's name when the text before `start` is `INTO <table> AS <alias>`, which Postgres and
/// SQLite (since 3.24) take: the rows are inserted after the alias, as after the table.
fn after_into_aliased(sql: &str, start: usize, opaque: &[(usize, usize)]) -> Option<(usize, usize)> {
    let b = sql.as_bytes();
    let mut i = start;
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    // The alias, bare or quoted.
    if i > 0 && (b[i - 1] == b'"' || b[i - 1] == b'`') {
        i = opaque.iter().find(|&&(_, e)| e == i)?.0;
    } else {
        let end = i;
        while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$') {
            i -= 1;
        }
        if i == end || NOT_ALIAS.iter().any(|k| sql[i..end].eq_ignore_ascii_case(k)) {
            return None;
        }
    }
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    let as_word = i >= 2 && sql[i - 2..i].eq_ignore_ascii_case("as") && (i == 2 || !(b[i - 3].is_ascii_alphanumeric() || b[i - 3] == b'_'));
    if !as_word {
        return None;
    }
    after_into(sql, i - 2, opaque)
}

/// The parenthesized list right before `start` when it follows `INTO <table>`: the author's own
/// column list before a rows cast, as `(start, end)`.
fn own_column_list(sql: &str, start: usize, opaque: &[(usize, usize)]) -> Option<(usize, usize)> {
    let b = sql.as_bytes();
    let mut i = start;
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 || b[i - 1] != b')' {
        return None;
    }
    let end = i;
    let mut depth = 0usize;
    let mut k = i;
    while k > 0 {
        if let Some(&(s, _)) = opaque.iter().find(|&&(_, e)| e == k) {
            k = s;
            continue;
        }
        match b[k - 1] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    let open = k - 1;
                    return (after_into(sql, open, opaque).is_some() || after_into_aliased(sql, open, opaque).is_some()).then_some((open, end));
                }
            }
            _ => {}
        }
        k -= 1;
    }
    None
}

/// The words that may follow a table without being its alias.
const NOT_ALIAS: &[&str] = &[
    "where", "join", "inner", "left", "right", "full", "cross", "natural", "on", "using", "group", "order", "having", "limit", "offset", "fetch", "for", "union", "except", "intersect", "window", "returning", "set", "lateral", "tablesample", "with", "into", "values", "select", "straight_join", "use", "force", "ignore", "partition",
];

/// An alias after a table expression ending at `end`: `AS name`, or a bare name that is not one of
/// the words that follow a table; `(the alias as written, its end)`.
fn alias_after(sql: &str, end: usize) -> Option<(String, usize)> {
    let b = sql.as_bytes();
    let mut i = skip_space(b, end);
    let word = |i: usize| i + b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_' || **c == b'$').count();
    let mut explicit = false;
    let w = word(i);
    if w > i && sql[i..w].eq_ignore_ascii_case("as") {
        explicit = true;
        i = skip_space(b, w);
    }
    match b.get(i) {
        Some(b'"') | Some(b'`') => {
            let q = b[i];
            let close = i + 1 + b[i + 1..].iter().position(|&c| c == q)?;
            Some((sql[i..=close].to_string(), close + 1))
        }
        Some(c) if c.is_ascii_alphabetic() || *c == b'_' => {
            let w = word(i);
            let name = &sql[i..w];
            if !explicit && NOT_ALIAS.iter().any(|k| name.eq_ignore_ascii_case(k)) {
                return None;
            }
            Some((name.to_string(), w))
        }
        _ => None,
    }
}

/// What the expansion writes for one parameter, with the places the rest of the pass needs, as
/// offsets into `text`.
#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub text: String,
    /// Where the author's placeholder sits in `text`.
    pub placeholder: (usize, usize),
    /// Per member, the places in `text` that stand for it (its column in each list it is in).
    pub members: Vec<Vec<(usize, usize)>>,
    /// In insert position, the insert's column list and the query's select list, which the run
    /// time writes again without the members whose keys a call leaves out.
    pub lists: Vec<List>,
}

/// One comma-separated list of the expansion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub start: usize,
    pub end: usize,
    /// Per member in descriptor order, its entry's text.
    pub entries: Vec<String>,
}

fn quote(name: &str, dialect: Dialect) -> String {
    match dialect {
        Dialect::Mysql => format!("`{}`", name.replace('`', "``")),
        _ => format!("\"{}\"", name.replace('"', "\"\"")),
    }
}

/// A JSON path to one key, quoted so any key works: `'$."id"'` as an SQL string.
fn path(name: &str) -> String {
    let key = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!("'$.\"{}\"'", key.replace('\'', "''"))
}

/// The alias of the table the expansion writes: the author's, or the parameter's name (its last
/// segment, for a dotted name).
fn alias_of(found: &Found, dialect: Dialect) -> String {
    match &found.alias {
        Some(a) => a.clone(),
        None => quote(found.name.rsplit('.').next().unwrap_or(&found.name), dialect),
    }
}

/// The default copied for member `k` of `found`, if a row that leaves its key out takes one.
fn copied_at(found: &Found, k: usize) -> Option<&str> {
    found.copied.as_ref()?.get(k)?.as_deref()
}

/// A Postgres string literal that reads the same whatever `standard_conforming_strings` says.
fn pg_literal(s: &str) -> String {
    if s.contains('\\') {
        format!("E'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

struct Writer {
    text: String,
    members: Vec<Vec<(usize, usize)>>,
    placeholder: (usize, usize),
}

impl Writer {
    fn new(n: usize) -> Writer {
        Writer { text: String::new(), members: vec![Vec::new(); n], placeholder: (0, 0) }
    }
    fn push(&mut self, s: &str) {
        self.text.push_str(s);
    }
    fn member(&mut self, k: usize, s: &str) {
        let start = self.text.len();
        self.text.push_str(s);
        self.members[k].push((start, self.text.len()));
    }
    fn placeholder(&mut self, s: &str) {
        let start = self.text.len();
        self.text.push_str(s);
        self.placeholder = (start, self.text.len());
    }
    /// A comma-separated list of one entry per member, recorded as a list the run time may write
    /// again.
    fn list(&mut self, entries: Vec<String>, lists: &mut Vec<List>) {
        let start = self.text.len();
        for (k, e) in entries.iter().enumerate() {
            if k > 0 {
                self.push(", ");
            }
            self.member(k, e);
        }
        lists.push(List { start, end: self.text.len(), entries });
    }
}

/// Whether a member holds binary data, which the JSON carries as hexadecimal text and the reader
/// decodes with `unhex()`, as a binary value travels in a text form the statement decodes
/// (wiki/typed-json-params.md). Postgres's reader takes `bytea` from its own `\x` text as it is.
fn binary(m: &Member, dialect: Dialect) -> bool {
    match dialect {
        Dialect::Postgres => false,
        Dialect::Sqlite => m.schema.is_none() && affinity(&m.ty) == "BLOB",
        Dialect::Mysql => {
            let t = m.ty.to_ascii_lowercase();
            ["binary", "varbinary", "tinyblob", "blob", "mediumblob", "longblob"].iter().any(|b| t == *b || t.starts_with(&format!("{b}(")))
        }
    }
}

/// The JSON column type of a JSON member in each dialect's reader.
fn json_type(member: &Member, dialect: Dialect) -> String {
    match dialect {
        Dialect::Postgres => member.ty.clone(),
        _ => "json".to_string(),
    }
}

/// ` WHERE t."a" IS NOT NULL AND …` over the `not null` members, or nothing.
fn not_null_filter(ms: &[Member], table: &str, dialect: Dialect) -> String {
    let tests: Vec<String> = ms.iter().filter(|m| m.not_null).map(|m| format!("{table}.{} IS NOT NULL", quote(&m.name, dialect))).collect();
    if tests.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", tests.join(" AND "))
    }
}

/// Whether a MySQL member is text, whose JSON_TABLE column takes the connection's collation.
fn text_member(m: &Member) -> bool {
    let t = mysql_type(&m.ty);
    let base = t.split(|c: char| c == '(' || c == ' ').next().unwrap_or("");
    m.schema.is_none() && matches!(base, "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum" | "set")
}

/// What the expansion writes for `found`, standing where the author put it.
pub fn lower(found: &Found, dialect: Dialect, placeholder: &str, server: &Server) -> Lowered {
    // A typed JSON parameter is the dialect's JSON parameter, its schema left out: Postgres
    // receives `$1::jsonb`, MySQL `cast(? as json)` and SQLite `?` (wiki/typed-json-params.md).
    if let Some(cast) = &found.plain {
        let (before, after) = cast.around.clone().unwrap_or_default();
        return Lowered { placeholder: (before.len(), before.len() + placeholder.len()), text: format!("{before}{placeholder}{after}"), members: Vec::new(), lists: Vec::new() };
    }
    if let Some(cast) = &found.json {
        let (before, after) = match dialect {
            Dialect::Postgres => ("", if cast.jsonb { "::jsonb" } else { "::json" }),
            // MariaDB has no `CAST(… AS JSON)`.
            Dialect::Mysql if server.mariadb => ("JSON_COMPACT(", ")"),
            Dialect::Mysql => ("cast(", " as json)"),
            Dialect::Sqlite => ("", ""),
        };
        let text = format!("{before}{placeholder}{after}");
        return Lowered { placeholder: (before.len(), before.len() + placeholder.len()), text, members: Vec::new(), lists: Vec::new() };
    }
    let ms = &found.members;
    let alias = alias_of(found, dialect);
    let mut w = Writer::new(ms.len());
    let mut lists = Vec::new();
    let q = |n: &str| quote(n, dialect);
    match dialect {
        Dialect::Postgres => {
            let coldefs = |w: &mut Writer| {
                for (k, m) in ms.iter().enumerate() {
                    if k > 0 {
                        w.push(", ");
                    }
                    w.member(k, &format!("{} {}", q(&m.name), m.ty));
                }
            };
            let reader = |w: &mut Writer| {
                w.push(if found.one { "jsonb_to_record(" } else { "jsonb_to_recordset(" });
                w.placeholder(placeholder);
                w.push("::jsonb)");
            };
            if found.insert {
                // A member whose key a row may leave out reads, for such a row, the default copied
                // from the catalog; `->` tells a missing key (SQL NULL) from a JSON null.
                let select = ms
                    .iter()
                    .enumerate()
                    .map(|(k, m)| {
                        let value = format!("{alias}.{}", q(&m.name));
                        match copied_at(found, k) {
                            Some(default) => format!("CASE WHEN (\"reylo.e\".\"reylo.v\" -> {}) IS NOT NULL THEN {value} ELSE {default} END", pg_literal(&m.name)),
                            None => value,
                        }
                    })
                    .collect();
                w.push("(");
                w.list(ms.iter().map(|m| q(&m.name)).collect(), &mut lists);
                w.push(") SELECT ");
                w.list(select, &mut lists);
                if found.one && (0..ms.len()).any(|k| copied_at(found, k).is_some()) {
                    // The object itself, which the key tests read.
                    w.push(" FROM (SELECT ");
                    w.placeholder(placeholder);
                    w.push(&format!("::jsonb AS \"reylo.v\") AS \"reylo.e\", jsonb_to_record(\"reylo.e\".\"reylo.v\") AS {alias}("));
                    coldefs(&mut w);
                    w.push(")");
                } else if found.one {
                    w.push(" FROM jsonb_to_record(");
                    w.placeholder(placeholder);
                    w.push(&format!("::jsonb) AS {alias}("));
                    coldefs(&mut w);
                    w.push(")");
                } else {
                    // The element's number decides the order the rows are inserted in.
                    w.push(" FROM jsonb_array_elements(");
                    w.placeholder(placeholder);
                    w.push(&format!("::jsonb) WITH ORDINALITY AS \"reylo.e\"(\"reylo.v\", \"reylo.n\"), jsonb_to_record(\"reylo.e\".\"reylo.v\") AS {alias}("));
                    coldefs(&mut w);
                    w.push(") ORDER BY \"reylo.e\".\"reylo.n\"");
                }
            } else if ms.iter().any(|m| m.not_null) {
                // The run time refuses a null for a `not null` key, so filtering on it removes
                // nothing; it lets the analysis read those columns as never null.
                w.push("(SELECT ");
                for (k, m) in ms.iter().enumerate() {
                    if k > 0 {
                        w.push(", ");
                    }
                    w.member(k, &format!("\"reylo.r\".{}", q(&m.name)));
                }
                w.push(" FROM ");
                reader(&mut w);
                w.push(" AS \"reylo.r\"(");
                coldefs(&mut w);
                w.push(&format!("){}) AS {alias}", not_null_filter(ms, "\"reylo.r\"", dialect)));
            } else {
                reader(&mut w);
                w.push(&format!(" AS {alias}("));
                coldefs(&mut w);
                w.push(")");
            }
        }
        Dialect::Sqlite => {
            let source = |w: &mut Writer| {
                if found.one {
                    w.push("(SELECT ");
                    w.placeholder(placeholder);
                    w.push(" AS value) AS \"reylo.e\"");
                } else {
                    w.push("json_each(");
                    w.placeholder(placeholder);
                    w.push(") AS \"reylo.e\"");
                }
            };
            // A JSON member keeps its JSON text; an explicit null is SQL NULL, not the text
            // `null` (wiki/structured-params/verify.md).
            let read = |m: &Member| {
                if m.schema.is_some() {
                    format!("nullif(\"reylo.e\".value -> {}, 'null')", path(&m.name))
                } else if binary(m, dialect) {
                    format!("unhex(\"reylo.e\".value ->> {})", path(&m.name))
                } else {
                    format!("\"reylo.e\".value ->> {}", path(&m.name))
                }
            };
            if found.insert {
                w.push("(");
                w.list(ms.iter().map(|m| q(&m.name)).collect(), &mut lists);
                w.push(") SELECT ");
                // As on Postgres, a member whose key a row may leave out reads the copied default
                // for such a row; json_type() is NULL for a missing key and 'null' for a JSON null.
                let select = ms
                    .iter()
                    .enumerate()
                    .map(|(k, m)| match copied_at(found, k) {
                        Some(default) => format!("CASE WHEN json_type(\"reylo.e\".value, {}) IS NOT NULL THEN {} ELSE {default} END", path(&m.name), read(m)),
                        None => read(m),
                    })
                    .collect();
                w.list(select, &mut lists);
                w.push(" FROM ");
                source(&mut w);
                // `WHERE` keeps a following ON CONFLICT from reading as a join's ON.
                w.push(if found.one { " WHERE true" } else { " WHERE true ORDER BY \"reylo.e\".key" });
            } else {
                // As on Postgres, a filter on the `not null` keys, which removes nothing, lets the
                // analysis read them as never null; it reads a name, so the values are named first.
                let filtered = ms.iter().any(|m| m.not_null);
                w.push(if filtered { "(SELECT * FROM (SELECT " } else { "(SELECT " });
                for (k, m) in ms.iter().enumerate() {
                    if k > 0 {
                        w.push(", ");
                    }
                    let value = if m.schema.is_some() || binary(m, dialect) { read(m) } else { format!("CAST({} AS {})", read(m), m.ty) };
                    w.member(k, &format!("{value} AS {}", q(&m.name)));
                }
                w.push(" FROM ");
                source(&mut w);
                if filtered {
                    w.push(&format!(") AS \"reylo.r\"{}", not_null_filter(ms, "\"reylo.r\"", dialect)));
                }
                w.push(&format!(") AS {alias}"));
            }
        }
        Dialect::Mysql => {
            // A binary member is read as its hexadecimal text and decoded with UNHEX().
            let table = |w: &mut Writer, ordinal: bool, alias: &str| {
                w.push("JSON_TABLE(");
                w.placeholder(placeholder);
                w.push(if found.one { ", '$' COLUMNS (" } else { ", '$[*]' COLUMNS (" });
                if ordinal {
                    w.push("`reylo.n` FOR ORDINALITY, ");
                }
                for (k, m) in ms.iter().enumerate() {
                    if k > 0 {
                        w.push(", ");
                    }
                    let ty = if m.schema.is_some() { json_type(m, dialect) } else if binary(m, dialect) { "longtext".to_string() } else { m.ty.clone() };
                    w.member(k, &format!("{} {ty} PATH {} ERROR ON ERROR", q(&m.name), path(&m.name)));
                }
                w.push(&format!(")) AS {alias}"));
            };
            // What the query reads of a member: binary data decoded, and on MariaDB, whose JSON
            // is text, a JSON null as SQL NULL rather than the text `null` (a JSON string "null"
            // is the text `"null"`, so nothing else reads as it).
            let value = |m: &Member, alias: &str| {
                let column = format!("{alias}.{}", q(&m.name));
                if binary(m, dialect) {
                    format!("UNHEX({column})")
                } else if m.schema.is_some() && server.mariadb {
                    format!("NULLIF({column}, 'null')")
                } else {
                    column
                }
            };
            if found.insert {
                w.push("(");
                w.list(ms.iter().map(|m| q(&m.name)).collect(), &mut lists);
                w.push(") SELECT ");
                w.list(ms.iter().map(|m| value(m, &alias)).collect(), &mut lists);
                w.push(" FROM ");
                table(&mut w, !found.one, &alias);
                if !found.one {
                    w.push(&format!(" ORDER BY {alias}.`reylo.n`"));
                }
            } else {
                // A derived table over the JSON_TABLE, never the JSON_TABLE alone: MySQL refuses
                // a locking clause over a query that reads a table function ("Table r appears in
                // multiple locking clauses") and LATERAL before one. A text member is given the
                // database's collation explicitly, so that it compares with a column of any
                // collation, a literal and a parameter; the JSON_TABLE's own column has the
                // connection's (docs/structured-params.md, Positions).
                w.push("(SELECT ");
                for (k, m) in ms.iter().enumerate() {
                    if k > 0 {
                        w.push(", ");
                    }
                    let read = match &server.collation {
                        Some((charset, collation)) if text_member(m) && !server.mariadb => format!("CONVERT(`reylo.j`.{} USING {charset}) COLLATE {collation}", q(&m.name)),
                        _ => value(m, "`reylo.j`"),
                    };
                    w.member(k, &format!("{read} AS {}", q(&m.name)));
                }
                w.push(" FROM ");
                table(&mut w, false, "`reylo.j`");
                w.push(&format!(") AS {alias}"));
            }
        }
    }
    Lowered { text: w.text, placeholder: w.placeholder, members: w.members, lists }
}

/// The statement with every structured parameter written as its expansion, and the map back.
#[derive(Debug, Clone, PartialEq)]
pub struct Expanded {
    pub sql: String,
    pub found: Vec<Found>,
    /// Per parameter, its expansion and where it starts in `sql`.
    pub lowered: Vec<(Lowered, usize)>,
}

pub fn expand(sql: &str, found: Vec<Found>, dialect: Dialect, server: &Server) -> Expanded {
    let mut out = String::new();
    let mut last = 0;
    let mut lowered = Vec::new();
    for f in &found {
        out.push_str(&sql[last..f.region.0]);
        let l = lower(f, dialect, &sql[f.placeholder.0..f.placeholder.1], server);
        let at = out.len();
        out.push_str(&l.text);
        lowered.push((l, at));
        last = f.region.1;
    }
    out.push_str(&sql[last..]);
    Expanded { sql: out, found, lowered }
}

impl Expanded {
    /// The offset in the author's text that an offset in the expanded text stands for. Inside an
    /// expansion it is the member whose column it falls on, else the parameter's head.
    pub fn to_author(&self, pos: usize) -> (usize, usize) {
        let mut delta: isize = 0;
        for (f, (l, at)) in self.found.iter().zip(&self.lowered) {
            let end = at + l.text.len();
            if pos >= end {
                delta += (f.region.1 - f.region.0) as isize - l.text.len() as isize;
                continue;
            }
            if pos >= *at {
                let local = pos - at;
                for (k, places) in l.members.iter().enumerate() {
                    if places.iter().any(|&(s, e)| local >= s && local < e) {
                        let m = &f.members[k];
                        return (m.span.0, m.span.1);
                    }
                }
                return f.head;
            }
            break;
        }
        let p = (pos as isize + delta).max(0) as usize;
        (p, p + 1)
    }

    /// The expansion of the parameter whose placeholder sits at `start` in the expanded text.
    pub fn at_placeholder(&self, start: usize) -> Option<usize> {
        self.lowered.iter().position(|(l, at)| at + l.placeholder.0 == start)
    }
}

/// The FROM-position expansion of `found` as a statement of its own, which the analysis
/// describes to learn each member's type as the database names it, and the expansion inside it,
/// which starts after `PROBE`.
pub fn probe(found: &Found, dialect: Dialect, placeholder: &str, server: &Server) -> (String, Lowered) {
    let mut f = found.clone();
    f.insert = false;
    f.table = None;
    f.plain = None;
    f.alias = Some(quote("reylo.probe", dialect));
    for m in &mut f.members {
        m.optional = false;
    }
    let lowered = lower(&f, dialect, placeholder, server);
    (format!("{PROBE}{}", lowered.text), lowered)
}

const PROBE: &str = "SELECT * FROM ";

/// What the analysis keeps of a structured parameter in insert position beside its type: the
/// lists of the rewritten statement that the run time writes again for a call that leaves keys
/// out, as offsets into it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Info {
    pub name: String,
    pub lists: Vec<List>,
    /// On Postgres and SQLite, where the statement copies the defaults instead of being written
    /// again per call, the columns whose defaults it copied, for the freshness facts
    /// ([`crate::fresh`]). Not on the wire.
    #[serde(skip)]
    pub copied: Vec<String>,
}

/// Whether no structured parameter has lists the run time writes again, so that the analysis
/// carries none ([`Analysis::structured`]).
pub fn none_rewritten(infos: &[Info]) -> bool {
    infos.iter().all(|i| i.lists.is_empty())
}

/// The structured parameters whose lists the run time writes again, as the analysis carries them:
/// one whose statement copies its defaults has none.
pub fn rewritten_only<S: serde::Serializer>(infos: &[Info], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(infos.iter().filter(|i| !i.lists.is_empty()))
}

/// The diagnostic code of a column list Reylo cannot read, or one whose types the database
/// refuses, and of a cast to `nullable(…)` that the column the parameter is written to refuses.
pub const CODE: u32 = 20;

/// The analysis of a statement that may have structured parameters: what `inner` answers for the
/// statement with each one expanded, read back in the author's text, with each parameter typed by
/// its column list. `inner` analyzes a statement that has none, given the names of the parameters
/// cast to `nullable(…)`, which the caller may pass null for; `bare` is the analysis of the
/// author's statement that only reports the diagnostics it is given.
pub fn analyze<D: Describe>(sql: &str, dialect: Dialect, host: &Host<'_, D>, inner: &dyn Fn(&str, &[String]) -> Lazy<Analysis>, bare: &dyn Fn(Vec<Diagnostic>) -> Analysis) -> Lazy<Analysis> {
    let refused = |e: DescriptorError| Some(bare(vec![Diagnostic { message: e.message, start: e.start, length: e.end - e.start, code: CODE }]));
    let found = match find(sql, dialect, &Server::default()) {
        Ok(found) => found,
        Err(e) => return refused(e),
    };
    if found.is_empty() {
        return inner(sql, &[]);
    }
    // On MySQL the expansion depends on the server, asked once a session.
    let server = if dialect == Dialect::Mysql {
        match host.query(MYSQL_SERVER_QUERY, Vec::new(), Scope::Session)? {
            Ok(rows) => Server::of(&rows),
            Err(_) => Server::default(),
        }
    } else {
        Server::default()
    };
    let found = if server == Server::default() {
        found
    } else {
        match find(sql, dialect, &server) {
            Ok(found) => found,
            Err(e) => return refused(e),
        }
    };
    // Each member's type as the database names it, and the columns of each table an insert
    // writes, for the checks below and the defaults it copies, asked in one round.
    let probes: Vec<(Lowered, Lazy<Analysis>)> = found
        .iter()
        .map(|f| {
            if f.json.is_some() || f.plain.is_some() {
                return (Lowered { text: String::new(), placeholder: (0, 0), members: Vec::new(), lists: Vec::new() }, Some(crate::analysis::bare_of("", dialect, Vec::new())));
            }
            let (text, lowered) = probe(f, dialect, &sql[f.placeholder.0..f.placeholder.1], &server);
            (lowered, inner(&text, &[]))
        })
        .collect();
    let tables: Vec<Lazy<Vec<TableColumn>>> = found.iter().map(|f| table_columns(f, sql, dialect, &server, host)).collect();
    // On Postgres and SQLite an insert whose rows may leave keys out copies each such column's
    // default into the statement, which is then one text for every call, whatever keys its rows
    // give; the statement is analyzed with them, a round later. MySQL's run time writes the lists
    // again per call instead (wiki/structured-params.md, question 6).
    let mut found = found;
    let mut columns = Vec::with_capacity(tables.len());
    let copying = dialect != Dialect::Mysql && found.iter().any(|f| f.insert && f.json.is_none() && f.plain.is_none() && f.members.iter().any(Member::omittable));
    if copying {
        for t in &tables {
            columns.push(t.clone()?);
        }
        for (f, c) in found.iter_mut().zip(&columns) {
            f.copied = copies(f, c, dialect);
        }
    }
    let marked: Vec<String> = found.iter().filter(|f| f.nullable).map(|f| f.name.clone()).collect();
    let expanded = expand(sql, found, dialect, &server);
    let mut analysis = inner(&expanded.sql, &marked)?;
    let mut probed = Vec::with_capacity(probes.len());
    for (lowered, a) in probes {
        probed.push((lowered, a?));
    }
    if !copying {
        for t in tables {
            columns.push(t?);
        }
    }

    // The statement's diagnostics, at the author's text.
    for d in &mut analysis.diagnostics {
        let (start, _) = expanded.to_author(d.start);
        let (_, end) = expanded.to_author(d.start + d.length.max(1) - 1);
        d.start = start;
        d.length = end.saturating_sub(start).max(1);
    }

    for ((f, (lowered, probe)), table) in expanded.found.iter().zip(&probed).zip(&columns) {
        if f.nullable {
            if let Some(p) = analysis.params.iter_mut().find(|p| p.name == f.name) {
                p.nullable = true;
            }
        }
        // A `::type` written as the dialect's own cast: the analysis typed it; the run time checks
        // its value, since that cast converts what it cannot read. On Postgres, which reads `::`
        // itself, the one such cast is the type inside `::nullable(…)`, which the server checks.
        if let Some(cast) = &f.plain {
            if dialect == Dialect::Postgres {
                continue;
            }
            if let Some(p) = analysis.params.iter_mut().find(|p| p.name == f.name) {
                p.cast = Some(cast.written.clone());
                // A text cast on MySQL is the bare placeholder, which MySQL types as a string
                // and MariaDB does not type at all: the cast says it is text.
                let base = cast.written.split('(').next().unwrap_or(&cast.written);
                if dialect == Dialect::Mysql && cast.around.is_none() && mysql_text(base) && matches!(p.ty, Type::Unknown) {
                    p.ty = Type::Scalar { name: "text".to_string() };
                }
                // SQLite's `json(?)` takes JSON text, and the parameter is a JSON value, written
                // as its text as every MySQL and SQLite JSON parameter is.
                // MariaDB's JSON_COMPACT(?) takes it as text too.
                if (dialect == Dialect::Sqlite || server.mariadb) && matches!(cast.written.as_str(), "json" | "jsonb") {
                    p.ty = Type::Json { schema: None };
                    p.type_name = cast.written.clone();
                }
            }
            continue;
        }
        if let Some(cast) = &f.json {
            if let Some(p) = analysis.params.iter_mut().find(|p| p.name == f.name) {
                p.ty = Type::Json { schema: Some(crate::schema_type::type_schema(&input_view(&cast.schema), &crate::schema_type::REYLO)) };
                p.type_name = if cast.jsonb { "jsonb" } else { "json" }.to_string();
                p.schema = Some(cast.schema.clone());
            }
            continue;
        }
        // A type the database does not know, or another refusal of the probe, at the member.
        for d in &probe.diagnostics {
            let local = d.start.saturating_sub(PROBE.len());
            let at = lowered.members.iter().position(|places| places.iter().any(|&(s, e)| local >= s && local < e));
            let (start, end) = match at {
                Some(k) => f.members[k].type_span,
                None => f.head,
            };
            // The statement reports the same refusal at the member: the probe's place, the type,
            // is the more exact, so it is kept alone.
            analysis.diagnostics.retain(|other| !(other.message == d.message && other.start >= f.region.0 && other.start < f.region.1));
            analysis.diagnostics.push(Diagnostic { message: d.message.clone(), start, length: end - start, code: d.code });
        }
        let fields: Vec<RowField> = f
            .members
            .iter()
            .enumerate()
            .map(|(k, m)| RowField {
                name: m.name.clone(),
                ty: match &m.schema {
                    Some(schema) => Type::Json { schema: Some(crate::schema_type::type_schema(&input_view(schema), &crate::schema_type::REYLO)) },
                    // Binary data, whatever the reader's unhex() is typed: SQLite's analysis types
                    // no such call, and MySQL's binary types are the engine's `bytea`.
                    None if binary(m, dialect) => Type::Scalar { name: if dialect == Dialect::Sqlite { "blob" } else { "bytea" }.to_string() },
                    None => probe.columns.get(k).map(|c| c.ty.clone()).unwrap_or(Type::Unknown),
                },
                written: m.ty.clone(),
                nullable: !m.not_null,
                optional: m.omittable(),
                schema: m.schema.clone(),
            })
            .collect();
        // The column list against the table: only once the database took every type.
        if probe.diagnostics.is_empty() {
            analysis.diagnostics.extend(check_table(f, sql, &fields, table, dialect));
        }
        if let Some(p) = analysis.params.iter_mut().find(|p| p.name == f.name) {
            p.ty = Type::Rows { one: f.one, fields };
            p.type_name = if f.one { "row" } else { "rows" }.to_string();
            p.nullable = false;
        }
    }

    // Offsets of the expanded statement in the rewritten one: every placeholder before them
    // changed its length, and every text the rewrite added before them moved them.
    let spans = analysis.rewritten.spans.clone();
    let inserted = analysis.rewritten.inserted.clone();
    let rewritten = |p: usize| {
        let delta: isize = spans.iter().filter(|&&(_, oe, _, _)| oe <= p).map(|&(os, oe, rs, re)| (re - rs) as isize - (oe - os) as isize).sum();
        let added: usize = inserted.iter().filter(|&&(at, _, _)| at <= p).map(|&(_, rs, re)| re - rs).sum();
        (p as isize + delta) as usize + added
    };
    for ((f, (l, at)), table) in expanded.found.iter().zip(&expanded.lowered).zip(&columns) {
        if !f.insert {
            continue;
        }
        if f.copied.is_some() {
            // The columns whose defaults the statement copied, defaults or none, which the
            // freshness check compares: a default changed after the build would otherwise be
            // written as it was.
            let copied = f.members.iter().filter(|m| m.omittable()).filter_map(|m| column_of(table, &m.name, dialect)).map(|c| c.name.clone()).collect();
            analysis.structured.push(Info { name: f.name.clone(), lists: Vec::new(), copied });
        } else {
            let lists = l.lists.iter().map(|list| List { start: rewritten(at + list.start), end: rewritten(at + list.end), entries: list.entries.clone() }).collect();
            analysis.structured.push(Info { name: f.name.clone(), lists, copied: Vec::new() });
        }
    }
    // Each placeholder at its place in the author's text; a structured parameter's covers its
    // whole expansion, so an error the database reports inside it lands on the parameter.
    analysis.rewritten.spans = spans
        .iter()
        .map(|&(os, oe, rs, re)| match expanded.at_placeholder(os) {
            Some(k) => {
                let (l, at) = &expanded.lowered[k];
                let f = &expanded.found[k];
                (f.region.0, f.region.1, rewritten(*at), rewritten(at + l.text.len()))
            }
            None => {
                let start = expanded.to_author(os).0;
                (start, start + (oe - os), rs, re)
            }
        })
        .collect();
    analysis.rewritten.inserted = inserted.iter().map(|&(at, rs, re)| (expanded.to_author(at).0, rs, re)).collect();
    Some(analysis)
}

/// Where a schema's keyword holds subschemas: one, a list, or a map of them.
enum Sub {
    One,
    List,
    Map,
}

const SUBSCHEMAS: &[(&str, Sub)] = &[
    ("properties", Sub::Map),
    ("patternProperties", Sub::Map),
    ("additionalProperties", Sub::One),
    ("unevaluatedProperties", Sub::One),
    ("propertyNames", Sub::One),
    ("dependentSchemas", Sub::Map),
    ("items", Sub::One),
    ("prefixItems", Sub::List),
    ("contains", Sub::One),
    ("allOf", Sub::List),
    ("anyOf", Sub::List),
    ("oneOf", Sub::List),
    ("not", Sub::One),
    ("if", Sub::One),
    ("then", Sub::One),
    ("else", Sub::One),
];

const JSON_TYPES: &[&str] = &["string", "number", "integer", "boolean", "null", "object", "array"];

/// What the profile of wiki/typed-json-params.md refuses in a schema, as a message naming where:
/// a reference, which breaks the schema's locality; a type that depends on which keyword matched
/// or where validation entered, which no TypeScript type says; another draft's `$schema`;
/// `nullable`, which is not 2020-12; and the traps that make a schema refuse every value.
pub fn profile(schema: &Value, at: &str) -> Option<String> {
    let place = |key: &str| if at.is_empty() { format!("`{key}`") } else { format!("`{key}` at {at}") };
    let s = match schema {
        Value::Bool(_) => return None,
        Value::Object(s) => s,
        other => return Some(format!("A schema is an object, as in {{\"type\": \"string\"}}, or true or false; {} is {other}.", if at.is_empty() { "the schema".to_string() } else { at.to_string() })),
    };
    for key in ["$ref", "$defs", "definitions"] {
        if s.contains_key(key) {
            return Some(format!("{}: a reference breaks the schema's locality, so write the schema where it is used.", place(key)));
        }
    }
    for key in ["$dynamicRef", "$dynamicAnchor", "$recursiveRef", "$recursiveAnchor"] {
        if s.contains_key(key) {
            return Some(format!("{}: the same schema would mean a different type depending on where validation entered.", place(key)));
        }
    }
    if s.contains_key("unevaluatedItems") {
        return Some(format!("{}: an item's type would depend on which keyword checked it, which no TypeScript type says.", place("unevaluatedItems")));
    }
    if matches!(s.get("unevaluatedProperties"), Some(v) if !v.is_boolean()) {
        return Some(format!("{} with a schema types a key by which member matched, which no TypeScript type says; `false` closes the object.", place("unevaluatedProperties")));
    }
    if s.contains_key("$vocabulary") {
        return Some(format!("{} means nothing for a parameter.", place("$vocabulary")));
    }
    if s.contains_key("nullable") {
        return Some(format!("{} is not JSON Schema 2020-12: write \"type\": [T, \"null\"].", place("nullable")));
    }
    if at.is_empty() {
        if let Some(uri) = s.get("$schema") {
            if uri.as_str().map(crate::schema_type::Draft::named) != Some(Some(crate::schema_type::Draft::Twenty)) {
                return Some("`$schema` names another draft, whose keywords mean other things; the schema is JSON Schema 2020-12.".into());
            }
        }
    }
    let types: Vec<&str> = match s.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    if let Some(t) = types.iter().find(|t| !JSON_TYPES.contains(t)) {
        return Some(format!("{}: \"{t}\" is not one of JSON Schema's types (string, number, integer, boolean, null, object, array); a database type goes in the statement's column list or a row descriptor.", place("type")));
    }
    if s.get("additionalProperties") == Some(&Value::Bool(false)) && ["oneOf", "anyOf", "allOf"].iter().any(|k| s.contains_key(*k)) {
        return Some(format!("{} beside `oneOf`, `anyOf` or `allOf` sees only this object's own `properties`, so it refuses every member's keys: write `unevaluatedProperties: false`.", place("additionalProperties: false")));
    }
    if let Some(Value::Array(members)) = s.get("allOf") {
        if let Some(k) = members.iter().position(|m| m.get("additionalProperties") == Some(&Value::Bool(false))) {
            return Some(format!("Member {} of {} is closed, so it refuses every other member's keys: close the whole with `unevaluatedProperties: false`.", k + 1, place("allOf")));
        }
    }
    if let (Some(Value::Array(required)), Some(Value::Object(properties))) = (s.get("required"), s.get("properties")) {
        if let Some(key) = required.iter().filter_map(Value::as_str).find(|k| !properties.contains_key(*k)) {
            return Some(format!("{} lists \"{key}\", which `properties` does not name.", place("required")));
        }
    }
    for (key, kind) in SUBSCHEMAS {
        let Some(v) = s.get(*key) else { continue };
        let found = match kind {
            Sub::One => profile(v, &format!("{at}/{key}")),
            Sub::List => v.as_array().and_then(|l| l.iter().enumerate().find_map(|(i, x)| profile(x, &format!("{at}/{key}/{i}")))),
            Sub::Map => v.as_object().and_then(|m| m.iter().find_map(|(k, x)| profile(x, &format!("{at}/{key}/{k}")))),
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// The schema as the caller sees it: a key whose schema has a `default` may be left out, since the
/// run time fills it before the checks, so it leaves `required`. The
/// walk follows where the run time fills defaults, as ajv's `useDefaults` does: not inside
/// `anyOf`, `oneOf`, `not`, `if`, `contains` or `propertyNames`.
pub fn input_view(schema: &Value) -> Value {
    let Value::Object(s) = schema else { return schema.clone() };
    let mut out = s.clone();
    if let (Some(Value::Object(properties)), Some(Value::Array(required))) = (s.get("properties"), s.get("required")) {
        let kept: Vec<Value> = required.iter().filter(|k| k.as_str().is_none_or(|k| properties.get(k).and_then(|p| p.get("default")).is_none())).cloned().collect();
        out.insert("required".into(), Value::Array(kept));
    }
    for (key, kind) in SUBSCHEMAS {
        if matches!(*key, "anyOf" | "oneOf" | "not" | "if" | "contains" | "propertyNames") {
            continue;
        }
        let Some(v) = s.get(*key) else { continue };
        let walked = match kind {
            Sub::One => input_view(v),
            Sub::List => Value::Array(v.as_array().map(|l| l.iter().map(input_view).collect()).unwrap_or_default()),
            Sub::Map => Value::Object(v.as_object().map(|m| m.iter().map(|(k, x)| (k.clone(), input_view(x))).collect()).unwrap_or_default()),
        };
        out.insert((*key).to_string(), walked);
    }
    Value::Object(out)
}

/// The diagnostic code of a column list that disagrees with the table its insert writes.
pub const TABLE_CODE: u32 = 21;

/// One column of the table an insert writes, as its catalog says.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    pub name: String,
    /// Postgres: the base type's name (a domain's base); MySQL: `column_type`; SQLite: the
    /// declared type.
    pub base: String,
    /// The type as the table writes it, modifiers included.
    pub written: String,
    pub not_null: bool,
    /// A default of its own or its domain's, an identity, an AUTO_INCREMENT, or SQLite's rowid.
    pub has_default: bool,
    pub generated: bool,
    /// Postgres's `GENERATED ALWAYS AS IDENTITY`, which an insert may not set.
    pub always: bool,
    /// On Postgres and SQLite, the default as the insert writes it for a row that leaves the
    /// column's key out ([`copy_of`]); `None` for a column with none, whose missing key's NULL is
    /// what the database writes.
    pub default: Option<String>,
    /// A Postgres view's column with no default of its own, where an insert that leaves it out
    /// takes the default of the view's table, which the catalog does not give with the view.
    pub inherited: bool,
}

/// A Postgres column's default as SQL text (`pg_attribute a`, `pg_attrdef d`, `pg_type t` of the
/// column's type), or NULL where it has none, by the rules wiki/structured-params/default-extraction.md
/// measured on Postgres 14, 17 and 18: the column's own default, else for an identity
/// `GENERATED BY DEFAULT` its sequence's `nextval`, else its domain's default; a generated
/// column has none. `pg_get_expr` writes a name without its schema only where the session's
/// `search_path` finds the same object by it, so the text is read under the session's own path,
/// and the freshness check reads it again under the path the query runs with: equal text is the
/// same objects (wiki/design.md §24).
macro_rules! pg_default {
    () => {
        "CASE WHEN a.attgenerated <> '' THEN NULL \
        WHEN d.adbin IS NOT NULL THEN pg_catalog.pg_get_expr(d.adbin, d.adrelid) \
        WHEN a.attidentity = 'd' THEN 'nextval(' || pg_catalog.quote_literal(pg_catalog.pg_get_serial_sequence(a.attrelid::pg_catalog.regclass::text, a.attname::text)) || '::regclass)' \
        WHEN t.typtype = 'd' THEN pg_catalog.pg_get_expr(t.typdefaultbin, 0) END"
    };
}
pub(crate) use pg_default;

/// Every column of a Postgres table: `$1` is its name as the statement writes it. Beside the
/// facts the checks read, the default ([`pg_default`]), the type it is cast to, without
/// modifiers, as `format_type` writes a type for a cast, and whether the column is a view's with
/// no default of its own.
pub const PG_TABLE_QUERY: &str = concat!(
    "SELECT a.attname::text, b.typname::text, \
    pg_catalog.format_type(b.oid, CASE WHEN t.typtype = 'd' THEN t.typtypmod ELSE a.atttypmod END), \
    a.attnotnull OR (t.typtype = 'd' AND t.typnotnull), \
    a.atthasdef OR a.attidentity <> '' OR (t.typtype = 'd' AND t.typdefault IS NOT NULL), \
    a.attgenerated <> '', a.attidentity = 'a', ",
    pg_default!(),
    ", pg_catalog.format_type(a.atttypid, NULL), c.relkind = 'v' AND d.adbin IS NULL \
    FROM pg_catalog.pg_attribute a \
    JOIN pg_catalog.pg_class c ON c.oid = a.attrelid \
    JOIN pg_catalog.pg_type t ON t.oid = a.atttypid \
    JOIN pg_catalog.pg_type b ON b.oid = CASE WHEN t.typtype = 'd' THEN t.typbasetype ELSE t.oid END \
    LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
    WHERE a.attrelid = pg_catalog.to_regclass($1) AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum"
);

/// Every column of a SQLite table: its name and its schema.
pub const SQLITE_TABLE_QUERY: &str = "select name, type, \"notnull\", dflt_value, pk, hidden, (select count(*) from pragma_table_xinfo(?1, ?2) where pk > 0) from pragma_table_xinfo(?1, ?2)";

/// Every column of a MySQL table: its schema (the session's, when null) and its name.
pub const MYSQL_TABLE_QUERY: &str = "select column_name, data_type, column_type, is_nullable = 'NO', \
    column_default is not null or extra like '%auto_increment%', extra like '%VIRTUAL GENERATED%' or extra like '%STORED GENERATED%' \
    from information_schema.columns where table_schema = coalesce(?, database()) and table_name = ? order by ordinal_position";

/// The same on MariaDB, where a JSON column is LONGTEXT with a `json_valid` CHECK MariaDB adds
/// itself: such a column reads as `json`, the type it was declared with.
pub const MARIADB_TABLE_QUERY: &str = "select column_name, if(is_json, 'json', data_type), if(is_json, 'json', column_type), not_null, has_default, generated \
    from (select c.column_name, c.data_type, c.column_type, c.ordinal_position, c.is_nullable = 'NO' as not_null, \
    c.column_default is not null or c.extra like '%auto_increment%' as has_default, c.extra like '%VIRTUAL GENERATED%' or c.extra like '%STORED GENERATED%' as generated, \
    c.data_type = 'longtext' and exists (select 1 from information_schema.check_constraints k where k.constraint_schema = c.table_schema \
    and k.table_name = c.table_name and lower(k.check_clause) = concat('json_valid(`', lower(c.column_name), '`)')) as is_json \
    from information_schema.columns c where c.table_schema = coalesce(?, database()) and c.table_name = ?) x order by ordinal_position";

/// A table name as written, its quotes taken off: `(schema, name)`.
fn split_name(written: &str) -> (Option<String>, String) {
    let parts: Vec<String> = written
        .split('.')
        .map(|p| {
            let p = p.trim();
            match p.chars().next() {
                Some(q @ ('"' | '`')) if p.len() >= 2 => p[1..p.len() - 1].replace(&format!("{q}{q}"), &q.to_string()),
                _ => p.to_string(),
            }
        })
        .collect();
    match parts.as_slice() {
        [schema, name] => (Some(schema.clone()), name.clone()),
        _ => (None, parts.last().cloned().unwrap_or_default()),
    }
}

/// The columns of the table `f` inserts into, or none for a parameter elsewhere or a table the
/// catalog does not answer for (the statement's own errors say what is wrong then).
fn table_columns<D: Describe>(f: &Found, sql: &str, dialect: Dialect, server: &Server, host: &Host<'_, D>) -> Lazy<Vec<TableColumn>> {
    let Some((start, end)) = f.table else { return Some(Vec::new()) };
    let written = &sql[start..end];
    let rows = match dialect {
        Dialect::Postgres => host.query(PG_TABLE_QUERY, vec![serde_json::json!(written)], Scope::Statement)?,
        Dialect::Sqlite => {
            let (schema, name) = split_name(written);
            host.query(SQLITE_TABLE_QUERY, vec![serde_json::json!(name), serde_json::json!(schema.unwrap_or_else(|| "main".into()))], Scope::Statement)?
        }
        Dialect::Mysql => {
            let (schema, name) = split_name(written);
            host.query(if server.mariadb { MARIADB_TABLE_QUERY } else { MYSQL_TABLE_QUERY }, vec![schema.map_or(Value::Null, Value::String), serde_json::json!(name)], Scope::Statement)?
        }
    };
    let Ok(rows) = rows else { return Some(Vec::new()) };
    let text = |r: &[Value], k: usize| r.get(k).and_then(as_str).unwrap_or_default().to_string();
    let flag = |r: &[Value], k: usize| r.get(k).and_then(|v| as_bool(v).or_else(|| v.as_i64().map(|n| n != 0))).unwrap_or(false);
    Some(
        rows.iter()
            .map(|r| match dialect {
                Dialect::Postgres => {
                    let default = r.get(7).and_then(as_str).map(|e| copy_of(e, &text(r, 8), dialect));
                    TableColumn { name: text(r, 0), base: text(r, 1), written: text(r, 2), not_null: flag(r, 3), has_default: flag(r, 4), generated: flag(r, 5), always: flag(r, 6), default, inherited: flag(r, 9) }
                }
                Dialect::Mysql => TableColumn { name: text(r, 0), base: text(r, 1), written: text(r, 2), not_null: flag(r, 3), has_default: flag(r, 4), generated: flag(r, 5), always: false, default: None, inherited: false },
                Dialect::Sqlite => {
                    let declared = text(r, 1);
                    let pk = r.get(4).and_then(Value::as_i64).unwrap_or(0);
                    let hidden = r.get(5).and_then(Value::as_i64).unwrap_or(0);
                    let keys = r.get(6).and_then(Value::as_i64).unwrap_or(0);
                    // An INTEGER PRIMARY KEY is the rowid, which SQLite fills when it is left out.
                    let rowid = pk > 0 && keys == 1 && declared.eq_ignore_ascii_case("integer");
                    let default = match r.get(3) {
                        None | Some(Value::Null) => None,
                        Some(Value::String(e)) => Some(copy_of(e, "", dialect)),
                        Some(other) => Some(copy_of(&other.to_string(), "", dialect)),
                    };
                    TableColumn { name: text(r, 0), base: declared.clone(), written: declared, not_null: flag(r, 2), has_default: default.is_some() || rowid, generated: hidden == 2 || hidden == 3, always: false, default, inherited: false }
                }
            })
            .collect(),
    )
}

/// A default from the catalog as the insert writes it, by the rules
/// wiki/structured-params/default-extraction.md measured. On Postgres it is cast to the column's
/// type (`cast`, without modifiers): `pg_get_expr` leaves out the coercion the column applies, and
/// a CASE whose ELSE has another type changes the caller's own value (`DEFAULT CURRENT_USER`, a
/// `name`, cut a 100-character text to 63) or is refused (`text DEFAULT 1`); a cast to the type
/// without its modifiers leaves the length or precision to the column, which refuses a default too
/// long for it as the database does. On SQLite it is the default's source text, in parentheses
/// (which `dflt_value` drops), and a default written as a bare identifier, which SQLite stores as
/// a string, is written as one: as text it would read as a column.
fn copy_of(expr: &str, cast: &str, dialect: Dialect) -> String {
    match dialect {
        Dialect::Postgres => format!("({expr})::{cast}"),
        _ => format!("({})", sqlite_identifier_default(expr).unwrap_or_else(|| expr.to_string())),
    }
}

/// A SQLite default written as one identifier, `member`, `"member"`, `[member]` or `` `member` ``,
/// as the string SQLite stores for it; `None` for any other default. `true` and `false` bare are
/// SQLite's truth values, and `null` and the `current_*` keywords are no identifiers.
fn sqlite_identifier_default(expr: &str) -> Option<String> {
    let e = expr.trim();
    let name = if let Some(inner) = e.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        if inner.replace("\"\"", "").contains('"') {
            return None;
        }
        inner.replace("\"\"", "\"")
    } else if let Some(inner) = e.strip_prefix('`').and_then(|r| r.strip_suffix('`')) {
        if inner.replace("``", "").contains('`') {
            return None;
        }
        inner.replace("``", "`")
    } else if let Some(inner) = e.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        if inner.contains(']') {
            return None;
        }
        inner.to_string()
    } else {
        let bare = e.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()) && e.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$' || !c.is_ascii());
        if !bare || ["true", "false", "null", "current_time", "current_date", "current_timestamp"].iter().any(|k| e.eq_ignore_ascii_case(k)) {
            return None;
        }
        e.to_string()
    };
    Some(format!("'{}'", name.replace('\'', "''")))
}

/// Per member of an insert's column list on Postgres and SQLite, the default a row that leaves
/// its key out takes, copied from `columns`, the catalog's columns of the table, on the
/// databases where the catalog gives it with high fidelity. `None` where the catalog gave no column, so that the run time
/// writes the lists again per call, as on MySQL.
pub fn copies(f: &Found, columns: &[TableColumn], dialect: Dialect) -> Option<Vec<Option<String>>> {
    if dialect == Dialect::Mysql || !f.insert || f.json.is_some() || f.plain.is_some() || columns.is_empty() {
        return None;
    }
    Some(f.members.iter().map(|m| if m.omittable() { column_of(columns, &m.name, dialect).and_then(|c| c.default.clone()) } else { None }).collect())
}

/// The column named `name`, as the dialect compares names.
fn column_of<'a>(columns: &'a [TableColumn], name: &str, dialect: Dialect) -> Option<&'a TableColumn> {
    columns.iter().find(|c| if dialect == Dialect::Postgres { c.name == name } else { c.name.eq_ignore_ascii_case(name) })
}

/// The column list against the table its insert writes: a key that may be null goes to a column
/// that may hold NULL, a key the caller may leave out to a column with a default, every column
/// that needs a value is in the list, and each member's type is its column's, so that the run
/// time's checks are the column's own (the descriptor is explicit and checked against the table).
fn check_table(f: &Found, sql: &str, fields: &[RowField], columns: &[TableColumn], dialect: Dialect) -> Vec<Diagnostic> {
    let Some((ts, te)) = f.table else { return Vec::new() };
    if columns.is_empty() {
        return Vec::new();
    }
    let table = &sql[ts..te];
    let same = |a: &str, b: &str| if dialect == Dialect::Postgres { a == b } else { a.eq_ignore_ascii_case(b) };
    let mut out = Vec::new();
    let mut at = |span: (usize, usize), message: String| out.push(Diagnostic { message, start: span.0, length: span.1 - span.0, code: TABLE_CODE });
    for (m, field) in f.members.iter().zip(fields) {
        let Some(c) = columns.iter().find(|c| same(&c.name, &m.name)) else { continue };
        if c.generated {
            at(m.name_span, format!("{table}.{} is a generated column, which an insert cannot set: leave it out of the column list.", c.name));
            continue;
        }
        if c.always {
            at(m.name_span, format!("{table}.{} is GENERATED ALWAYS AS IDENTITY, which an insert cannot set: leave it out of the column list, so the database numbers the rows.", c.name));
            continue;
        }
        if c.inherited && m.omittable() {
            at(
                m.name_span,
                format!("{table}.{} is a column of a view with no default of its own, so a row that leaves `{}` out would take the default of the view's table, which Reylo does not copy into the insert: give the view's column a default (ALTER VIEW {table} ALTER COLUMN {} SET DEFAULT …), or write the member without `?` and with `not null`.", c.name, m.name, c.name),
            );
        } else if !m.not_null && c.not_null {
            let fix = if c.has_default { format!("`{}? {} not null`, which lets a row leave the key out so that the default applies", m.name, m.ty) } else { format!("`{} {} not null`", m.name, m.ty) };
            at(m.span, format!("{table}.{} is not null, and `{}` lets a row give null or leave the key out: write {fix}.", c.name, m.name));
        } else if m.optional && !c.has_default {
            at(m.name_span, format!("`{}?` lets a row leave the key out, but {table}.{} has no default, so such a row would fail: drop the `?`, or give the column a default.", m.name, c.name));
        }
        if let Some(message) = type_differs(m, field, c, dialect) {
            at(m.type_span, format!("{table}.{} is {}, and the column list says {}: {message}", c.name, c.written, m.ty));
        }
    }
    for c in columns {
        if c.not_null && !c.has_default && !c.generated && !f.members.iter().any(|m| same(&c.name, &m.name)) {
            at(f.head, format!("{table}.{} is not null and has no default, and the column list leaves it out, so every insert would fail: add `{} <type> not null` to the list.", c.name, c.name));
        }
    }
    out
}

/// Why a member's type is not its column's, if it is not.
fn type_differs(m: &Member, field: &RowField, c: &TableColumn, dialect: Dialect) -> Option<String> {
    let advice = "a column list writes each column's own type, so that the checks the run time makes are the column's.";
    match dialect {
        Dialect::Postgres => {
            let base = match &field.ty {
                Type::Scalar { name } | Type::Enum { name, .. } => name.clone(),
                Type::Array { of, .. } => match of.as_ref() {
                    Type::Scalar { name } | Type::Enum { name, .. } => format!("_{name}"),
                    _ => return None,
                },
                Type::Json { .. } => m.ty.clone(),
                _ => return None,
            };
            (base != c.base || modifiers(&m.ty) != modifiers(&c.written)).then(|| advice.to_string())
        }
        Dialect::Mysql => {
            if m.schema.is_some() {
                return (!c.base.eq_ignore_ascii_case("json")).then(|| "a `json(…)` member writes a JSON column.".to_string());
            }
            (mysql_type(&m.ty) != mysql_type(&c.written)).then(|| advice.to_string())
        }
        // SQLite keeps a value by the column's affinity; a JSON member is text to it.
        Dialect::Sqlite => (m.schema.is_none() && affinity(&m.ty) != affinity(&c.written)).then(|| format!("their affinities differ ({} and {}), and {advice}", affinity(&m.ty), affinity(&c.written))),
    }
}

/// A type's modifiers, `(12,2)` of `numeric(12,2)`, spaces dropped.
fn modifiers(written: &str) -> String {
    match (written.find('('), written.find(')')) {
        (Some(s), Some(e)) if e > s => written[s + 1..e].chars().filter(|c| !c.is_whitespace()).collect(),
        _ => String::new(),
    }
}

/// A MySQL type in one spelling: `integer` is `int`, `boolean` is `tinyint(1)`, `numeric` is
/// `decimal(10,0)`, an integer's display width (but `tinyint(1)`'s) and a fraction of `(0)` go.
fn mysql_type(written: &str) -> String {
    let mut t: String = written.to_ascii_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    t = t.replace(" (", "(").replace("( ", "(").replace(" )", ")").replace(", ", ",").replace(" ,", ",");
    for (from, to) in [("character varying", "varchar"), ("double precision", "double"), ("character", "char"), ("integer", "int"), ("numeric", "decimal"), ("fixed", "decimal")] {
        if t.starts_with(from) {
            t = format!("{to}{}", &t[from.len()..]);
        }
    }
    if t == "bool" || t == "boolean" {
        return "tinyint(1)".into();
    }
    if t.starts_with("dec") && !t.starts_with("decimal") {
        t = format!("decimal{}", &t[3..]);
    }
    if t == "decimal" || t.starts_with("decimal ") {
        t = t.replacen("decimal", "decimal(10,0)", 1);
    }
    if let Some(rest) = t.strip_prefix("decimal(") {
        if let Some((p, tail)) = rest.split_once(')') {
            if !p.contains(',') {
                t = format!("decimal({p},0){tail}");
            }
        }
    }
    for int in ["tinyint", "smallint", "mediumint", "int", "bigint"] {
        if let Some(rest) = t.strip_prefix(&format!("{int}(")) {
            if let Some((width, tail)) = rest.split_once(')') {
                if !(int == "tinyint" && width == "1") {
                    t = format!("{int}{tail}");
                }
            }
        }
    }
    for time in ["datetime", "timestamp", "time"] {
        if t == format!("{time}(0)") {
            t = time.to_string();
        }
    }
    if t == "char" {
        t = "char(1)".into();
    }
    if t == "real" {
        t = "double".into();
    }
    t
}

/// SQLite's affinity of a declared type, by its rules (https://sqlite.org/datatype3.html, 3.1).
fn affinity(declared: &str) -> &'static str {
    let t = declared.to_ascii_uppercase();
    if t.contains("INT") {
        "INTEGER"
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        "TEXT"
    } else if t.contains("BLOB") || t.trim().is_empty() {
        "BLOB"
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        "REAL"
    } else {
        "NUMERIC"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_is_whether_the_pass_rewrites_the_statement() {
        for dialect in [Dialect::Postgres, Dialect::Mysql, Dialect::Sqlite] {
            assert!(present("select * from :r::rows(id int8)", dialect), "{dialect:?}");
            assert!(present("select :doc::jsonb('{\"type\": \"object\"}')", dialect), "{dialect:?}");
            assert!(present("select :q::nullable(text)", dialect), "{dialect:?}");
            assert!(!present("select :q", dialect), "{dialect:?}");
        }
        // A plain `::type` is Postgres's own cast, and the pass writes it on the others alone.
        assert!(!present("select :n::int", Dialect::Postgres));
        assert!(present("select :n::int", Dialect::Mysql));
        assert!(present("select :n::int", Dialect::Sqlite));
    }

    fn one(sql: &str, dialect: Dialect) -> Found {
        let mut found = find(sql, dialect, &Server::default()).expect("reads");
        assert_eq!(found.len(), 1, "{found:?}");
        found.remove(0)
    }

    fn err(sql: &str, dialect: Dialect) -> String {
        find(sql, dialect, &Server::default()).expect_err("refused").message
    }

    #[test]
    fn a_column_list_is_written_bare_and_never_as_a_string() {
        let sql = "insert into events :events::rows(id int8 not null, status? text not null, amount numeric(12, 2), tags text [], meta jsonb('{\"type\": \"object\"}')) returning id";
        let f = one(sql, Dialect::Postgres);
        assert!(f.insert && !f.one);
        let shape: Vec<_> = f.members.iter().map(|m| (m.name.as_str(), m.ty.as_str(), m.not_null, m.optional)).collect();
        assert_eq!(shape, vec![("id", "int8", true, false), ("status", "text", true, true), ("amount", "numeric(12,2)", false, false), ("tags", "text[]", false, false), ("meta", "jsonb", false, false)]);
        assert_eq!(f.members[4].schema, Some(serde_json::json!({"type": "object"})));
        assert_eq!(&sql[f.region.0..f.region.1], &sql[19..sql.len() - " returning id".len()]);
        assert_eq!(&sql[f.head.0..f.head.1], ":events::rows");
        assert!(err("insert into events :events::rows('id int8 not null')", Dialect::Postgres).contains("written bare in the parentheses"));
    }

    #[test]
    fn offsets_in_a_column_list_point_into_the_author_text() {
        let sql = "insert into t :p::rows(a int8 not null, b jsonb('{\"const\": \"it''s\"}'))";
        let f = one(sql, Dialect::Postgres);
        assert_eq!(&sql[f.members[0].name_span.0..f.members[0].name_span.1], "a");
        assert_eq!(&sql[f.members[1].type_span.0..f.members[1].type_span.1], "jsonb('{\"const\": \"it''s\"}')");
        assert_eq!(f.members[1].schema, Some(serde_json::json!({"const": "it's"})));
        let f = one("select :p::jsonb('{\"const\": \"it''s\"}')", Dialect::Postgres);
        assert_eq!(f.json.map(|c| c.schema), Some(serde_json::json!({"const": "it's"})));
        assert!(err("select :p::jsonb('{}' || '')", Dialect::Postgres).contains("only thing in the parentheses"));
        assert!(err("select :p::jsonb(x)", Dialect::Postgres).contains("takes one string"));
    }

    #[test]
    fn a_cast_on_mysql_and_sqlite_is_the_dialects_own() {
        let my = |sql: &str| expand(sql, find(sql, Dialect::Mysql, &Server::default()).unwrap(), Dialect::Mysql, &Server::default()).sql;
        assert_eq!(
            my("select id from events where amount > :min::numeric(10, 2) and id = :id :: int8 order by id limit :n::int"),
            "select id from events where amount > CAST(:min AS DECIMAL(10,2)) and id = CAST(:id AS SIGNED) order by id limit :n"
        );
        assert_eq!(my("select 1 from t limit :a::int, :b::int"), "select 1 from t limit :a, :b");
        assert_eq!(my("select 1 from t limit 5 offset :o::bigint"), "select 1 from t limit 5 offset :o");
        assert_eq!(my("select :a::text, :b::varchar(20), :c::datetime, :d::double precision, :e::json, :f::int unsigned, ?::date, :u::uuid"), "select :a, :b, CAST(:c AS DATETIME(6)), CAST(:d AS DOUBLE), CAST(:e AS JSON), CAST(:f AS UNSIGNED), CAST(? AS DATE), :u");
        // A text cast is the bare placeholder, which takes the collation of the column it meets,
        // where a CAST to CHAR would take the connection's and fail the comparison; the
        // parameter keeps its cast, which the run time checks.
        let sql = "select name from users where name like concat(:prefix::text, '%') and code = :c::char(3) and nick = ? :: character varying and id = :u::uuid";
        assert_eq!(my(sql), "select name from users where name like concat(:prefix, '%') and code = :c and nick = ? and id = :u");
        let casts: Vec<_> = find(sql, Dialect::Mysql, &Server::default()).unwrap().into_iter().map(|f| f.plain.map(|c| (c.written, c.around))).collect();
        assert_eq!(casts, [Some(("text".to_string(), None)), Some(("char(3)".to_string(), None)), Some(("character varying".to_string(), None)), Some(("uuid".to_string(), None))]);
        assert!(err("select :p::bool", Dialect::Mysql).contains("no boolean type"));
        assert!(err("select :p::timestamptz", Dialect::Mysql).contains("drops the zone"));
        assert!(err("select :p::interval", Dialect::Mysql).contains("has no type for `::interval`"));
        assert!(err("select :p::int[]", Dialect::Mysql).contains("no arrays"));
        assert!(err("select 1 from t limit :n::text", Dialect::Mysql).contains("take an integer"));
        assert_eq!(my("select 1 from t where a = ?::int or b = ? :: smallint"), "select 1 from t where a = CAST(? AS SIGNED) or b = CAST(? AS SIGNED)");
        let lite = |sql: &str| expand(sql, find(sql, Dialect::Sqlite, &Server::default()).unwrap(), Dialect::Sqlite, &Server::default()).sql;
        assert_eq!(lite("select * from t where n > :n::int8 limit :k::int"), "select * from t where n > CAST(:n AS INTEGER) limit CAST(:k AS INTEGER)");
        assert_eq!(lite("select json_extract(:doc::json, '$.a'), :s::varchar(3), :f::double precision, :m::decimal(10, 2), :b::blob"), "select json_extract(json(:doc), '$.a'), CAST(:s AS TEXT), CAST(:f AS REAL), CAST(:m AS NUMERIC), CAST(:b AS BLOB)");
        // SQLite's CAST to `date` would read '2026-10-12' as 2026; it keeps a date as text.
        assert_eq!(lite("select * from t where born > :d::date and at < :t::timestamptz"), "select * from t where born > CAST(:d AS TEXT) and at < CAST(:t AS TEXT)");
        assert!(err("select :p::boolean", Dialect::Sqlite).contains("no boolean type"));
        assert!(err("select :p::money", Dialect::Sqlite).contains("no type `money`"));
        assert!(err("select ?1::text[]", Dialect::Sqlite).contains("no arrays"));
        // Postgres reads `::` itself.
        assert!(find("select :p::int8 limit :n::int", Dialect::Postgres, &Server::default()).unwrap().is_empty());
        // A `::` inside a string or after something other than a parameter is no cast of one.
        assert!(find("select ':p::int', col from t", Dialect::Mysql, &Server::default()).unwrap().is_empty());
    }

    #[test]
    fn a_cast_to_nullable_is_written_as_the_cast_inside() {
        let expanded = |sql: &str, dialect: Dialect| expand(sql, find(sql, dialect, &Server::default()).unwrap(), dialect, &Server::default()).sql;
        let marked = |sql: &str, dialect: Dialect| find(sql, dialect, &Server::default()).unwrap().into_iter().filter(|f| f.nullable).map(|f| f.name).collect::<Vec<_>>();
        // Postgres keeps the type as written: a schema's, a quoted name, two words, a modifier.
        let sql = "select :a::nullable(text), :b::Nullable( timestamp with time zone ), :c::nullable(public.\"Mood\"), :d::nullable(int[]), :e::nullable(numeric(10, 2)), :f::int";
        assert_eq!(expanded(sql, Dialect::Postgres), "select :a::text, :b::timestamp with time zone, :c::public.\"Mood\", :d::int[], :e::numeric(10, 2), :f::int");
        assert_eq!(marked(sql, Dialect::Postgres), ["a", "b", "c", "d", "e"]);
        // MySQL and SQLite write the type inside as they write `::type`, by where it stands too.
        assert_eq!(expanded("select :n::nullable(int), :s::nullable(text) from t limit :k::nullable(int)", Dialect::Mysql), "select CAST(:n AS SIGNED), :s from t limit :k");
        assert_eq!(expanded("select :n::nullable(int8), :d::nullable(date)", Dialect::Sqlite), "select CAST(:n AS INTEGER), CAST(:d AS TEXT)");
        // A typed JSON parameter inside, with its schema.
        let f = one("select :doc::nullable(jsonb('{\"type\": \"object\"}'))", Dialect::Postgres);
        assert!(f.nullable);
        assert_eq!(f.json.map(|c| c.schema), Some(serde_json::json!({"type": "object"})));
        assert_eq!(expanded("select :doc::nullable(json('{\"type\": \"object\"}'))", Dialect::Mysql), "select cast(:doc as json)");
        // What it does not take.
        assert!(err("select :p::nullable()", Dialect::Postgres).contains("takes the parameter's type"));
        assert!(err("select :p::nullable('text')", Dialect::Postgres).contains("not a string"));
        assert!(err("select :p::nullable(nullable(text))", Dialect::Postgres).contains("once"));
        assert!(err("select * from :p::nullable(rows(id int8))", Dialect::Postgres).contains("never null"));
        assert!(err("select :p::nullable(int int)", Dialect::Mysql).contains("one type alone"));
        // On Postgres, the text is written after `::` as it stands, so it is one type: a name, its
        // modifiers, array bounds and the words of Postgres's own types of several words.
        for written in ["int", "public.mood", "\"My Type\"", "varchar(255)", "numeric(10, 2)", "int[]", "int[3]", "int array", "timestamp with time zone", "timestamp(3) without time zone", "double precision", "character varying(10)", "interval day to second", "bit varying", "public.\"Mood\"[]", "pg_catalog.int4"] {
            let sql = format!("select :p::nullable({written})");
            assert_eq!(expanded(&sql, Dialect::Postgres), format!("select :p::{written}"), "{written}");
        }
        for written in ["int, 2", "int or true", "int and x", "text collate \"C\"", "timestamp at time zone 'utc'", "int + 1", "int[a]", "int.", ".int", "int int", "(int)"] {
            let sql = format!("select :p::nullable({written})");
            assert!(err(&sql, Dialect::Postgres).contains("one type alone"), "{written}");
        }
        assert!(err("select :p::nullable(bool)", Dialect::Mysql).contains("no boolean type"));
    }

    #[test]
    fn position_is_insert_only_right_after_into_a_table() {
        assert!(one("INSERT INTO public.events :e::rows(id int8)", Dialect::Postgres).insert);
        assert!(one("insert into \"Events\" :e::rows(id int8)", Dialect::Postgres).insert);
        assert!(one("insert ignore into `db`.`events` :e::rows(id bigint)", Dialect::Mysql).insert);
        assert!(one("insert or replace into events :e::rows(id integer)", Dialect::Sqlite).insert);
        let f = one("select e.id from :e::rows(id int8) as e where e.id > 1", Dialect::Postgres);
        assert!(!f.insert);
        assert_eq!(f.alias.as_deref(), Some("e"));
        let f = one("select * from :e::rows(id int8) where id > 1", Dialect::Postgres);
        assert_eq!(f.alias, None);
        let f = one("select * from :e::rows(id int8) x join t on t.id = x.id", Dialect::Postgres);
        assert_eq!(f.alias.as_deref(), Some("x"));
    }

    #[test]
    fn a_question_mark_in_a_bare_list_is_not_a_placeholder() {
        for dialect in [Dialect::Mysql, Dialect::Sqlite] {
            let f = one("insert into accounts :accounts::rows(name text not null, role? text not null)", dialect);
            assert_eq!(f.members.len(), 2);
            assert!(f.members[1].optional);
        }
    }

    #[test]
    fn what_a_column_list_refuses() {
        let pg = Dialect::Postgres;
        assert!(err("insert into t :p::rows(a text not null default 'x')", pg).contains("never restates a default"));
        assert!(err("insert into t :p::rows(a int8 primary key)", pg).contains("belongs to the table"));
        assert!(err("insert into t :p::rows(a? text)", pg).contains("may be left out already"));
        assert!(err("insert into t :p::rows(a text, a int8)", pg).contains("twice"));
        assert!(err("insert into t :p::rows()", pg).contains("empty"));
        assert!(err("insert into t :p::rows(a)", pg).contains("has no type"));
        assert!(err("insert into t :p::rows(a jsonb('{'))", pg).contains("is not JSON"));
        assert!(err("insert into t :p::row(a text, posts rows(title text))", pg).contains("nested"));
        assert!(err("select * from :p::rows(a? text not null)", pg).contains("only where the rows are inserted"));
        assert!(err("insert into t $1::rows(a text)", pg).contains("named"));
    }

    #[test]
    fn postgres_lowers_by_position_in_the_arrays_order() {
        let sql = "insert into events :events::rows(id int8 not null, status? text not null) on conflict (id) do nothing returning id";
        let found = find(sql, Dialect::Postgres, &Server::default()).unwrap();
        let e = expand(sql, found, Dialect::Postgres, &Server::default());
        assert_eq!(
            e.sql,
            "insert into events (\"id\", \"status\") SELECT \"events\".\"id\", \"events\".\"status\" FROM jsonb_array_elements(:events::jsonb) WITH ORDINALITY AS \"reylo.e\"(\"reylo.v\", \"reylo.n\"), jsonb_to_record(\"reylo.e\".\"reylo.v\") AS \"events\"(\"id\" int8, \"status\" text) ORDER BY \"reylo.e\".\"reylo.n\" on conflict (id) do nothing returning id"
        );
        let sql = "select e.id from :events::rows(id int8 not null, note text) e";
        let e = expand(sql, find(sql, Dialect::Postgres, &Server::default()).unwrap(), Dialect::Postgres, &Server::default());
        assert_eq!(e.sql, "select e.id from (SELECT \"reylo.r\".\"id\", \"reylo.r\".\"note\" FROM jsonb_to_recordset(:events::jsonb) AS \"reylo.r\"(\"id\" int8, \"note\" text) WHERE \"reylo.r\".\"id\" IS NOT NULL) AS e");
        let sql = "select * from :events::rows(note text)";
        let e = expand(sql, find(sql, Dialect::Postgres, &Server::default()).unwrap(), Dialect::Postgres, &Server::default());
        assert_eq!(e.sql, "select * from jsonb_to_recordset(:events::jsonb) AS \"events\"(\"note\" text)");
        let sql = "insert into users :user::row(email text not null)";
        let e = expand(sql, find(sql, Dialect::Postgres, &Server::default()).unwrap(), Dialect::Postgres, &Server::default());
        assert_eq!(e.sql, "insert into users (\"email\") SELECT \"user\".\"email\" FROM jsonb_to_record(:user::jsonb) AS \"user\"(\"email\" text)");
    }

    #[test]
    fn sqlite_and_mysql_lower_with_their_readers() {
        let sql = "insert into events :events::rows(id integer not null, meta json('{}'))";
        let e = expand(sql, find(sql, Dialect::Sqlite, &Server::default()).unwrap(), Dialect::Sqlite, &Server::default());
        assert_eq!(
            e.sql,
            "insert into events (\"id\", \"meta\") SELECT \"reylo.e\".value ->> '$.\"id\"', nullif(\"reylo.e\".value -> '$.\"meta\"', 'null') FROM json_each(:events) AS \"reylo.e\" WHERE true ORDER BY \"reylo.e\".key"
        );
        let sql = "insert into events :events::rows(id bigint not null, ts datetime(6))";
        let e = expand(sql, find(sql, Dialect::Mysql, &Server::default()).unwrap(), Dialect::Mysql, &Server::default());
        assert_eq!(
            e.sql,
            "insert into events (`id`, `ts`) SELECT `events`.`id`, `events`.`ts` FROM JSON_TABLE(:events, '$[*]' COLUMNS (`reylo.n` FOR ORDINALITY, `id` bigint PATH '$.\"id\"' ERROR ON ERROR, `ts` datetime(6) PATH '$.\"ts\"' ERROR ON ERROR)) AS `events` ORDER BY `events`.`reylo.n`"
        );
        let sql = "select * from :p::rows(n integer) as x";
        let e = expand(sql, find(sql, Dialect::Sqlite, &Server::default()).unwrap(), Dialect::Sqlite, &Server::default());
        assert_eq!(e.sql, "select * from (SELECT CAST(\"reylo.e\".value ->> '$.\"n\"' AS integer) AS \"n\" FROM json_each(:p) AS \"reylo.e\") AS x");
    }

    fn mysql_8() -> Server {
        Server::of(&[vec![serde_json::json!("8.4.11"), serde_json::json!("utf8mb4"), serde_json::json!("utf8mb4_0900_ai_ci")]])
    }

    fn mariadb() -> Server {
        Server::of(&[vec![serde_json::json!("11.8.9-MariaDB"), serde_json::json!("utf8mb4"), serde_json::json!("utf8mb4_general_ci")]])
    }

    #[test]
    fn the_server_is_read_from_its_version_and_the_databases_collation() {
        assert_eq!(mysql_8(), Server { mariadb: false, collation: Some(("utf8mb4".into(), "utf8mb4_0900_ai_ci".into())) });
        assert!(mariadb().mariadb);
        // A name that is not a plain collation name is never written into the statement.
        assert_eq!(Server::of(&[vec![serde_json::json!("8.4.11"), serde_json::json!("utf8mb4"), serde_json::json!("x) or (1")]]).collation, None);
        assert_eq!(Server::of(&[]), Server::default());
    }

    #[test]
    fn a_mysql_rows_table_is_a_derived_table_whose_text_has_the_databases_collation() {
        let sql = "select t.id from t join :p::rows(id bigint not null, x varchar(32) not null, data varbinary(8)) r on r.x = t.x for update";
        let my = |server: &Server| expand(sql, find(sql, Dialect::Mysql, server).unwrap(), Dialect::Mysql, server).sql;
        assert_eq!(
            my(&mysql_8()),
            "select t.id from t join (SELECT `reylo.j`.`id` AS `id`, CONVERT(`reylo.j`.`x` USING utf8mb4) COLLATE utf8mb4_0900_ai_ci AS `x`, UNHEX(`reylo.j`.`data`) AS `data` FROM JSON_TABLE(:p, '$[*]' COLUMNS (`id` bigint PATH '$.\"id\"' ERROR ON ERROR, `x` varchar(32) PATH '$.\"x\"' ERROR ON ERROR, `data` longtext PATH '$.\"data\"' ERROR ON ERROR)) AS `reylo.j`) AS r on r.x = t.x for update"
        );
        // Without the collation (a server that did not say), the column as it is.
        assert!(my(&Server::default()).contains("(SELECT `reylo.j`.`id` AS `id`, `reylo.j`.`x` AS `x`, "));
        // MariaDB compares its JSON_TABLE's text with any column already, and reads a JSON null as SQL NULL.
        let sql = "select r.meta from :p::rows(x text, meta json('{}')) r";
        assert_eq!(
            expand(sql, find(sql, Dialect::Mysql, &mariadb()).unwrap(), Dialect::Mysql, &mariadb()).sql,
            "select r.meta from (SELECT `reylo.j`.`x` AS `x`, NULLIF(`reylo.j`.`meta`, 'null') AS `meta` FROM JSON_TABLE(:p, '$[*]' COLUMNS (`x` text PATH '$.\"x\"' ERROR ON ERROR, `meta` json PATH '$.\"meta\"' ERROR ON ERROR)) AS `reylo.j`) AS r"
        );
        // The table is named by the parameter when no alias is written, and LATERAL takes it.
        let sql = "select t.id from t cross join lateral :p::rows(id bigint) where p.id = t.id";
        assert!(expand(sql, find(sql, Dialect::Mysql, &mysql_8()).unwrap(), Dialect::Mysql, &mysql_8()).sql.starts_with("select t.id from t cross join lateral (SELECT `reylo.j`.`id` AS `id` FROM JSON_TABLE(:p, '$[*]' COLUMNS (`id` bigint PATH '$.\"id\"' ERROR ON ERROR)) AS `reylo.j`) AS `p` where"));
    }

    #[test]
    fn a_mysql_insert_takes_the_row_alias_written_after_its_column_list() {
        let sql = "insert into t :p::rows(id bigint not null, x varchar(32) not null) as new on duplicate key update x = new.x";
        let f = one(sql, Dialect::Mysql);
        assert!(f.insert);
        assert_eq!(f.alias.as_deref(), Some("new"));
        assert_eq!(
            expand(sql, vec![f], Dialect::Mysql, &mysql_8()).sql,
            "insert into t (`id`, `x`) SELECT new.`id`, new.`x` FROM JSON_TABLE(:p, '$[*]' COLUMNS (`reylo.n` FOR ORDINALITY, `id` bigint PATH '$.\"id\"' ERROR ON ERROR, `x` varchar(32) PATH '$.\"x\"' ERROR ON ERROR)) AS new ORDER BY new.`reylo.n` on duplicate key update x = new.x"
        );
        // Without AS the next word is the statement's, and without an alias the parameter names the table.
        let f = one("insert into t :p::rows(id bigint) on duplicate key update id = p.id", Dialect::Mysql);
        assert_eq!(f.alias, None);
        // On MariaDB a JSON member is written as SQL NULL for a JSON null.
        let sql = "insert into t :p::row(meta json('{}'))";
        assert!(expand(sql, find(sql, Dialect::Mysql, &mariadb()).unwrap(), Dialect::Mysql, &mariadb()).sql.contains("SELECT NULLIF(`p`.`meta`, 'null') FROM JSON_TABLE"));
    }

    #[test]
    fn a_cast_where_mysql_takes_only_a_placeholder_is_the_bare_placeholder() {
        let my = |sql: &str, server: &Server| expand(sql, find(sql, Dialect::Mysql, server).unwrap(), Dialect::Mysql, server).sql;
        let m8 = mysql_8();
        assert_eq!(
            my("select sum(n) over (order by id rows between :a::int preceding and :b :: int following), ntile(:c::int) over w, lag(n, :d::int) over w, lead(coalesce(n, (1)), :e::bigint) over w from t", &m8),
            "select sum(n) over (order by id rows between :a preceding and :b following), ntile(:c) over w, lag(n, :d) over w, lead(coalesce(n, (1)), :e) over w from t"
        );
        assert_eq!(my("select sum(n) over (order by x range between :a::numeric(4,1) preceding and current row) from t", &m8), "select sum(n) over (order by x range between :a preceding and current row) from t");
        // A call that is not a window function keeps its cast, as does LAG's first argument.
        assert_eq!(my("select round(x, :a::int), concat(lag, :b::int), lag(:c::int, 1) over w from t", &m8), "select round(x, CAST(:a AS SIGNED)), concat(lag, CAST(:b AS SIGNED)), lag(CAST(:c AS SIGNED), 1) over w from t");
        let err = |sql: &str, server: &Server| find(sql, Dialect::Mysql, server).expect_err("refused").message;
        assert!(err("select ntile(:k::text) over w from t", &m8).contains("take an integer"));
        assert!(err("select sum(n) over (rows between :k::date preceding and current row) from t", &m8).contains("takes a number"));
        // MariaDB takes NTILE's and LAG's argument bare too, but no parameter as a frame's bound.
        assert_eq!(my("select ntile(:k::int) over w from t", &mariadb()), "select ntile(:k) over w from t");
        assert!(err("select sum(n) over (rows between :k::int preceding and current row) from t", &mariadb()).contains("only a literal"));
        // SQLite takes an expression in each.
        let sql = "select ntile(:k::int) over w from t";
        assert_eq!(expand(sql, find(sql, Dialect::Sqlite, &Server::default()).unwrap(), Dialect::Sqlite, &Server::default()).sql, "select ntile(CAST(:k AS INTEGER)) over w from t");
    }

    #[test]
    fn json_on_mariadb_is_read_by_json_compact() {
        let sql = "select json_extract(:d::json, '$.a'), :t::json('{\"type\": \"object\"}')";
        assert_eq!(expand(sql, find(sql, Dialect::Mysql, &mariadb()).unwrap(), Dialect::Mysql, &mariadb()).sql, "select json_extract(JSON_COMPACT(:d), '$.a'), JSON_COMPACT(:t)");
        assert_eq!(expand(sql, find(sql, Dialect::Mysql, &mysql_8()).unwrap(), Dialect::Mysql, &mysql_8()).sql, "select json_extract(CAST(:d AS JSON), '$.a'), cast(:t as json)");
    }

    #[test]
    fn an_insert_takes_the_tables_alias_and_refuses_a_column_list_of_its_own() {
        for dialect in [Dialect::Postgres, Dialect::Sqlite] {
            let sql = "insert into t as tt :p::rows(id int8 not null, x text not null) on conflict (id) do update set x = excluded.x where tt.x <> excluded.x";
            let f = one(sql, dialect);
            assert!(f.insert);
            assert_eq!(f.table.map(|(s, e)| &sql[s..e]), Some("t"));
            assert!(expand(sql, vec![f], dialect, &Server::default()).sql.starts_with("insert into t as tt (\"id\", \"x\") SELECT "));
        }
        for dialect in [Dialect::Postgres, Dialect::Mysql, Dialect::Sqlite] {
            let sql = "insert into t (id, x) :p::rows(id int8 not null, x text not null)";
            let e = find(sql, dialect, &Server::default()).expect_err("refused");
            assert!(e.message.contains("writes the insert's column list itself"), "{}", e.message);
            assert_eq!(&sql[e.start..e.end], "(id, x)");
        }
        // A subquery's parenthesis before a rows table is no column list.
        assert!(!one("select * from (select 1) a join :p::rows(id int8) r on true", Dialect::Postgres).insert);
    }

    #[test]
    fn a_typed_json_cast_is_the_dialects_json_parameter() {
        let sql = "insert into t select * from json_each(:p::json('{\"type\": \"array\"}'))";
        let f = one(sql, Dialect::Sqlite);
        assert_eq!(f.json.as_ref().map(|c| c.schema.clone()), Some(serde_json::json!({"type": "array"})));
        assert_eq!(expand(sql, vec![f], Dialect::Sqlite, &Server::default()).sql, "insert into t select * from json_each(:p)");
        let sql = "select * from jsonb_to_recordset(:p::jsonb('{\"type\": \"array\"}')) as x(a int)";
        assert_eq!(expand(sql, find(sql, Dialect::Postgres, &Server::default()).unwrap(), Dialect::Postgres, &Server::default()).sql, "select * from jsonb_to_recordset(:p::jsonb) as x(a int)");
        let sql = "select * from json_table(:p::json('{}'), '$[*]' columns (a int path '$.a')) as x";
        assert_eq!(expand(sql, find(sql, Dialect::Mysql, &Server::default()).unwrap(), Dialect::Mysql, &Server::default()).sql, "select * from json_table(cast(:p as json), '$[*]' columns (a int path '$.a')) as x");
        // A cast without a schema is an ordinary cast.
        assert!(find("select :p::jsonb", Dialect::Postgres, &Server::default()).unwrap().is_empty());
    }

    #[test]
    fn the_profile_refuses_what_no_typescript_type_says() {
        let refused = |schema: serde_json::Value| profile(&schema, "").unwrap_or_default();
        assert!(refused(serde_json::json!({"$defs": {}, "$ref": "#/$defs/a"})).contains("breaks the schema's locality"));
        assert!(refused(serde_json::json!({"type": "array", "items": {"$dynamicRef": "#a"}})).contains("at /items"));
        assert!(refused(serde_json::json!({"type": "array", "unevaluatedItems": false})).contains("which keyword checked it"));
        assert!(refused(serde_json::json!({"unevaluatedProperties": {"type": "string"}})).contains("by which member matched"));
        assert!(refused(serde_json::json!({"type": "string", "nullable": true})).contains("[T, \"null\"]"));
        assert!(refused(serde_json::json!({"type": "text"})).contains("not one of JSON Schema's types"));
        assert!(refused(serde_json::json!({"$schema": "http://json-schema.org/draft-07/schema#"})).contains("another draft"));
        assert!(refused(serde_json::json!({"oneOf": [{}], "additionalProperties": false})).contains("unevaluatedProperties: false"));
        assert!(refused(serde_json::json!({"type": "object", "properties": {"a": {}}, "required": ["b"]})).contains("lists \"b\""));
        assert_eq!(profile(&serde_json::json!({"type": "object", "properties": {"a": {"type": ["string", "null"]}}, "unevaluatedProperties": false, "prefixItems": [true]}), ""), None);
        // A key with a default may be left out by the caller; one inside a union member may not.
        assert_eq!(
            input_view(&serde_json::json!({"type": "object", "properties": {"a": {"default": 1}, "b": {}}, "required": ["a", "b"], "anyOf": [{"properties": {"c": {"default": 2}}, "required": ["c"]}]})),
            serde_json::json!({"type": "object", "properties": {"a": {"default": 1}, "b": {}}, "required": ["b"], "anyOf": [{"properties": {"c": {"default": 2}}, "required": ["c"]}]})
        );
        assert!(err("insert into t :p::rows(meta jsonb('{\"default\": {}}'))", Dialect::Postgres).contains("by omission"));
        assert!(err("insert into t :p::rows(meta jsonb('{\"$ref\": \"#\"}'))", Dialect::Postgres).contains("locality"));
        assert!(err("select :p::jsonb('{\"$ref\": \"#\"}')", Dialect::Postgres).contains("locality"));
    }

    #[test]
    fn types_compare_in_one_spelling() {
        assert_eq!(mysql_type("INTEGER"), "int");
        assert_eq!(mysql_type("int(11)"), "int");
        assert_eq!(mysql_type("bigint(20) unsigned"), "bigint unsigned");
        assert_eq!(mysql_type("boolean"), "tinyint(1)");
        assert_eq!(mysql_type("tinyint(1)"), "tinyint(1)");
        assert_eq!(mysql_type("numeric"), "decimal(10,0)");
        assert_eq!(mysql_type("DECIMAL( 12 , 2 )"), "decimal(12,2)");
        assert_eq!(mysql_type("dec(5)"), "decimal(5,0)");
        assert_eq!(mysql_type("character varying(32)"), "varchar(32)");
        assert_eq!(mysql_type("datetime(0)"), "datetime");
        assert_ne!(mysql_type("datetime(6)"), mysql_type("datetime"));
        assert_eq!(modifiers("numeric(12, 2)"), "12,2");
        assert_eq!(modifiers("timestamp(3) with time zone"), "3");
        assert_eq!(modifiers("text"), "");
        assert_eq!(affinity("VARCHAR(10)"), "TEXT");
        assert_eq!(affinity("bigint"), "INTEGER");
        assert_eq!(affinity("json"), "NUMERIC");
        assert_eq!(affinity(""), "BLOB");
        assert_eq!(split_name("\"app\".\"Events\""), (Some("app".into()), "Events".into()));
        assert_eq!(split_name("`db`.`t`"), (Some("db".into()), "t".into()));
        assert_eq!(split_name("events"), (None, "events".into()));
    }

    #[test]
    fn an_offset_in_the_expansion_maps_to_the_member_it_stands_for() {
        let sql = "insert into events :events::rows(id int8 not null, status? text not null) returning id";
        let e = expand(sql, find(sql, Dialect::Postgres, &Server::default()).unwrap(), Dialect::Postgres, &Server::default());
        let status = e.sql.find("\"status\"").unwrap();
        let (s, t) = e.to_author(status);
        assert_eq!(&sql[s..t], "status? text not null");
        let ret = e.sql.find("returning").unwrap();
        assert_eq!(e.to_author(ret).0, sql.find("returning").unwrap());
        let reader = e.sql.find("jsonb_array_elements").unwrap();
        let (s, t) = e.to_author(reader);
        assert_eq!(&sql[s..t], ":events::rows");
    }

    /// `columns` as the catalog query answers for a table: Postgres's ten columns, SQLite's seven.
    fn table(dialect: Dialect, columns: &[(&str, &str, bool, Option<&str>)]) -> Vec<Vec<Value>> {
        use serde_json::json;
        columns
            .iter()
            .map(|&(name, ty, not_null, default)| match dialect {
                Dialect::Postgres => vec![json!(name), json!(ty), json!(ty), json!(not_null), json!(default.is_some()), json!(false), json!(false), json!(default), json!(ty), json!(false)],
                _ => vec![json!(name), json!(ty), json!(not_null as i64), json!(default), json!(if name == "id" { 1 } else { 0 }), json!(0), json!(1)],
            })
            .collect()
    }

    /// The analysis of `sql` over a host whose table query answers `rows`, and every statement the
    /// analysis asked to have analyzed, the expanded statement last.
    fn drive(sql: &str, dialect: Dialect, rows: Vec<Vec<Value>>) -> (Analysis, Vec<String>) {
        use crate::api::{Answer, Answers, NoDescribe, Need};
        let mut answers: Answers<NoDescribe> = Answers::new();
        for _ in 0..4 {
            let host = Host::new(&answers);
            let seen = std::cell::RefCell::new(Vec::new());
            let inner = |s: &str, _: &[String]| {
                seen.borrow_mut().push(s.to_string());
                Some(crate::analysis::bare_of(s, dialect, Vec::new()))
            };
            if let Some(a) = analyze(sql, dialect, &host, &inner, &|d| crate::analysis::bare_of(sql, dialect, d)) {
                return (a, seen.into_inner());
            }
            for need in host.needs() {
                if let Need::Query { id, sql: q, .. } = need {
                    let answer = if q.contains("pg_attribute") || q.contains("pragma_table_xinfo") { rows.clone() } else { Vec::new() };
                    answers.insert(id, Answer::Rows { rows: answer });
                }
            }
        }
        panic!("the analysis of {sql} never settled");
    }

    #[test]
    fn postgres_and_sqlite_copy_each_omittable_columns_default_into_one_statement() {
        // Postgres: the default cast to the column's type, read only where the row leaves the key
        // out; a member with no default reads its value, whose missing key is NULL.
        let sql = "insert into accounts :a::rows(name text not null, role? text not null, email text, n? int8 not null) returning id";
        let rows = table(Dialect::Postgres, &[("id", "int8", true, Some("nextval('accounts_id_seq'::regclass)")), ("name", "text", true, None), ("role", "text", true, Some("'member'::text")), ("email", "text", false, None), ("n", "int8", true, Some("nextval('public.n_seq'::regclass)"))]);
        let (a, seen) = drive(sql, Dialect::Postgres, rows);
        assert_eq!(
            seen.last().unwrap(),
            "insert into accounts (\"name\", \"role\", \"email\", \"n\") SELECT \"a\".\"name\", CASE WHEN (\"reylo.e\".\"reylo.v\" -> 'role') IS NOT NULL THEN \"a\".\"role\" ELSE ('member'::text)::text END, \"a\".\"email\", CASE WHEN (\"reylo.e\".\"reylo.v\" -> 'n') IS NOT NULL THEN \"a\".\"n\" ELSE (nextval('public.n_seq'::regclass))::int8 END FROM jsonb_array_elements(:a::jsonb) WITH ORDINALITY AS \"reylo.e\"(\"reylo.v\", \"reylo.n\"), jsonb_to_record(\"reylo.e\".\"reylo.v\") AS \"a\"(\"name\" text, \"role\" text, \"email\" text, \"n\" int8) ORDER BY \"reylo.e\".\"reylo.n\" returning id"
        );
        // No list for the run time to write again, and the copied columns for the freshness facts.
        assert_eq!(a.structured, vec![Info { name: "a".into(), lists: Vec::new(), copied: vec!["role".into(), "email".into(), "n".into()] }]);
        assert!(!serde_json::to_value(&a).unwrap().as_object().unwrap().contains_key("structured"));

        // One row reads the object itself for its key tests.
        let sql = "insert into accounts :a::row(name text not null, role? text not null)";
        let rows = table(Dialect::Postgres, &[("name", "text", true, None), ("role", "text", true, Some("'member'::text"))]);
        let (_, seen) = drive(sql, Dialect::Postgres, rows);
        assert_eq!(seen.last().unwrap(), "insert into accounts (\"name\", \"role\") SELECT \"a\".\"name\", CASE WHEN (\"reylo.e\".\"reylo.v\" -> 'role') IS NOT NULL THEN \"a\".\"role\" ELSE ('member'::text)::text END FROM (SELECT :a::jsonb AS \"reylo.v\") AS \"reylo.e\", jsonb_to_record(\"reylo.e\".\"reylo.v\") AS \"a\"(\"name\" text, \"role\" text)");

        // SQLite: the default's source text in parentheses, a bare identifier as the string it is.
        let sql = "insert into accounts :a::rows(id? integer not null, name text not null, role? text not null, nick text, token? text not null)";
        let rows = table(Dialect::Sqlite, &[("id", "INTEGER", false, None), ("name", "TEXT", true, None), ("role", "TEXT", true, Some("'member'")), ("nick", "TEXT", false, Some("member")), ("token", "TEXT", true, Some("lower(hex(randomblob(4)))"))]);
        let (a, seen) = drive(sql, Dialect::Sqlite, rows);
        assert_eq!(
            seen.last().unwrap(),
            "insert into accounts (\"id\", \"name\", \"role\", \"nick\", \"token\") SELECT \"reylo.e\".value ->> '$.\"id\"', \"reylo.e\".value ->> '$.\"name\"', CASE WHEN json_type(\"reylo.e\".value, '$.\"role\"') IS NOT NULL THEN \"reylo.e\".value ->> '$.\"role\"' ELSE ('member') END, CASE WHEN json_type(\"reylo.e\".value, '$.\"nick\"') IS NOT NULL THEN \"reylo.e\".value ->> '$.\"nick\"' ELSE ('member') END, CASE WHEN json_type(\"reylo.e\".value, '$.\"token\"') IS NOT NULL THEN \"reylo.e\".value ->> '$.\"token\"' ELSE (lower(hex(randomblob(4)))) END FROM json_each(:a) AS \"reylo.e\" WHERE true ORDER BY \"reylo.e\".key"
        );
        assert_eq!(a.structured[0].copied, ["id", "role", "nick", "token"]);

        // MySQL: unchanged, the lists the run time writes again per call.
        let sql = "insert into accounts :a::rows(name varchar(9) not null, role? varchar(9) not null)";
        let (a, seen) = drive(sql, Dialect::Mysql, Vec::new());
        assert!(seen.last().unwrap().starts_with("insert into accounts (`name`, `role`) SELECT `a`.`name`, `a`.`role` FROM JSON_TABLE("), "{}", seen.last().unwrap());
        assert_eq!((a.structured.len(), a.structured[0].lists.len(), a.structured[0].copied.len()), (1, 2, 0));
        assert!(serde_json::to_value(&a).unwrap()["structured"].is_array());
    }

    #[test]
    fn text_the_dialects_rewrite_adds_before_a_list_moves_the_list_with_it() {
        // MySQL's rewrite may add text the author did not write (the MySQL engine's
        // `mysql_collation`): each list the run time writes again keeps covering its own text.
        use crate::api::{Answer, Answers, NoDescribe, Need};
        let sql = "insert into accounts :a::rows(name varchar(9) not null, role? varchar(9) not null)";
        let added = "/* added */ ";
        let run = |add: bool| {
            let mut answers: Answers<NoDescribe> = Answers::new();
            for _ in 0..4 {
                let host = Host::new(&answers);
                let inner = |s: &str, _: &[String]| {
                    let mut a = crate::analysis::bare_of(s, Dialect::Mysql, Vec::new());
                    if add {
                        a.rewritten.sql = format!("{added}{}", a.rewritten.sql);
                        a.rewritten.spans = a.rewritten.spans.iter().map(|&(os, oe, rs, re)| (os, oe, rs + added.len(), re + added.len())).collect();
                        a.rewritten.inserted = vec![(0, 0, added.len())];
                    }
                    Some(a)
                };
                if let Some(a) = analyze(sql, Dialect::Mysql, &host, &inner, &|d| crate::analysis::bare_of(sql, Dialect::Mysql, d)) {
                    return a;
                }
                for need in host.needs() {
                    if let Need::Query { id, .. } = need {
                        answers.insert(id, Answer::Rows { rows: Vec::new() });
                    }
                }
            }
            panic!("the analysis never settled");
        };
        let (plain, moved) = (run(false), run(true));
        let text = |a: &Analysis| a.structured[0].lists.iter().map(|l| a.rewritten.sql[l.start..l.end].to_string()).collect::<Vec<_>>();
        assert_eq!(plain.structured[0].lists.len(), 2);
        assert_eq!(text(&moved), text(&plain));
        assert_eq!(moved.rewritten.inserted, [(0, 0, added.len())]);
        assert_eq!(moved.rewritten.to_original(added.len() + 3), 3);
    }

    #[test]
    fn a_default_is_copied_by_each_databases_rules() {
        // Postgres: cast to the column's type without its modifiers, which the column applies.
        assert_eq!(copy_of("CURRENT_USER", "text", Dialect::Postgres), "(CURRENT_USER)::text");
        assert_eq!(copy_of("'abcdef'::character varying", "character varying", Dialect::Postgres), "('abcdef'::character varying)::character varying");
        // SQLite: a lone identifier is the string SQLite stores; keywords and expressions are not.
        for (written, copied) in [("member", "('member')"), ("\"member\"", "('member')"), ("[a b]", "('a b')"), ("`it``s`", "('it`s')"), ("o'k", "(o'k)"), ("\"it's\"", "('it''s')"), ("true", "(true)"), ("FALSE", "(FALSE)"), ("null", "(null)"), ("current_timestamp", "(current_timestamp)"), ("'member'", "('member')"), ("-7", "(-7)"), ("x'00ff'", "(x'00ff')"), ("datetime('now')", "(datetime('now'))"), ("1 + 1", "(1 + 1)")] {
            assert_eq!(copy_of(written, "", Dialect::Sqlite), copied, "{written}");
        }
        // A view's column with no default of its own is refused where a row may leave its key out.
        let sql = "insert into v :a::rows(name text not null, note text)";
        let mut rows = table(Dialect::Postgres, &[("name", "text", true, None), ("note", "text", false, None)]);
        rows[1][9] = serde_json::json!(true);
        let found = find(sql, Dialect::Postgres, &Server::default()).unwrap();
        let columns: Vec<TableColumn> = rows
            .iter()
            .map(|r| TableColumn { name: r[0].as_str().unwrap().into(), base: "text".into(), written: "text".into(), not_null: r[3] == true, has_default: false, generated: false, always: false, default: None, inherited: r[9] == true })
            .collect();
        let fields: Vec<RowField> = found[0].members.iter().map(|m| RowField { name: m.name.clone(), ty: Type::Scalar { name: "text".into() }, written: m.ty.clone(), nullable: !m.not_null, optional: m.omittable(), schema: None }).collect();
        let d = check_table(&found[0], sql, &fields, &columns, Dialect::Postgres);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].message.starts_with("v.note is a column of a view with no default of its own"), "{}", d[0].message);
    }

}
