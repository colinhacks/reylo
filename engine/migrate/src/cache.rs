//! The replay cache. A check replays a project's whole history in its shadows, and nearly all of its
//! time is those replays, so what a replay ends in is kept: the catalog's lines at a prefix of steps
//! (`.reylo/migrate/lines/`, for every database), and on Postgres and SQLite the database itself,
//! which a later shadow starts from to replay only what follows it. On Postgres that database is
//! `reylo_cache_<key>` on the shadows' server, made by renaming a shadow that is done with and
//! copied with `create database … template`; on SQLite it is a file in `.reylo/migrate/sqlite/`,
//! copied with SQLite's backup API. MySQL keeps lines alone: with no template databases, a copy is
//! made by running every object's `create` again, which costs what replaying a history of simple
//! migrations costs.
//!
//! A cache never changes an outcome. Its key covers everything that decides a replay's: this
//! build's code, the server's version, settings, roles and extensions, what a shadow is made with
//! (its encoding and locale, the target's database settings and default privileges), and the names
//! and texts of the steps, in order. A step that fails is never cached, so a prefix that is cached
//! ran whole. A shadow's session must also be as a new session leaves it wherever the cache stands
//! in for it (a migration that runs `set search_path` leaves the next one running elsewhere), so
//! nothing is kept or used past a point where the session differs from how it opened; and where any
//! text reads the session's sequence or random state, no shadow starts from a copy. `REYLO_NO_CACHE=1`
//! turns it off.

use crate::db::{literal, pg_close, pg_error, pg_rows, random_name, with_database, Db, Server, Shadow};
use crate::{Dialect, Error};
use regex::Regex;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub(crate) type Hash = [u8; 32];

pub(crate) const CACHE_PREFIX: &str = "reylo_cache_";

/// The database caches kept for each target: the newest by use. A check uses up to three (the
/// prefix it starts from, the prefix it ends at, `schema.sql`'s).
const KEEP: usize = 3;
/// The database caches kept on one server, whatever their targets.
const KEEP_ON_SERVER: usize = 32;
/// The lines kept in a project, and its SQLite databases.
const KEEP_LINES: usize = 16;
const KEEP_SQLITE: usize = 3;
/// What nobody has used for a week is dropped, as are the caches of a target that is gone.
const STALE_MS: u128 = 7 * 24 * 3600 * 1000;

// ---------------------------------------------------------------------------------------------
// SHA-256, for the keys: a cache that answered for the wrong history would change an outcome, so a
// key is a cryptographic digest of what it stands for.

pub(crate) struct Sha256 {
    h: [u32; 8],
    block: [u8; 64],
    used: usize,
    length: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    pub(crate) fn new() -> Sha256 {
        Sha256 { h: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19], block: [0; 64], used: 0, length: 0 }
    }

    fn compress(h: &mut [u32; 8], block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }

    pub(crate) fn update(&mut self, mut data: &[u8]) -> &mut Sha256 {
        self.length += data.len() as u64;
        while !data.is_empty() {
            let take = (64 - self.used).min(data.len());
            self.block[self.used..self.used + take].copy_from_slice(&data[..take]);
            self.used += take;
            data = &data[take..];
            if self.used == 64 {
                Sha256::compress(&mut self.h, &self.block);
                self.used = 0;
            }
        }
        self
    }

    /// One field of a key, its length first, so that no two lists of fields hash alike.
    pub(crate) fn part(&mut self, data: &[u8]) -> &mut Sha256 {
        self.update(&(data.len() as u64).to_be_bytes()).update(data)
    }

    pub(crate) fn finish(&mut self) -> Hash {
        let bits = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.used != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 32];
        for (i, word) in self.h.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

pub(crate) fn hex(h: &[u8]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// This build's catalog reader, which decides what a schema's fingerprint is a digest of: a build
/// whose reader differs reads another fingerprint for the same schema, which is then not drift.
pub(crate) fn code_id() -> String {
    static ID: LazyLock<String> = LazyLock::new(|| {
        let mut h = Sha256::new();
        h.part(include_str!("catalog.rs").as_bytes());
        hex(&h.finish()[..6])
    });
    ID.clone()
}

/// This build's code, as far as it decides what a replay ends in or what the catalog reads: the
/// version, and the sources that make shadows, run their steps, connect to their servers and read
/// their catalogs. A development build edited between two runs keys its caches anew.
static CODE: LazyLock<Hash> = LazyLock::new(|| {
    let mut h = Sha256::new();
    for source in [
        include_str!("../../../Cargo.toml"),
        include_str!("../Cargo.toml"),
        include_str!("lib.rs"),
        include_str!("cache.rs"),
        include_str!("replay.rs"),
        include_str!("commands.rs"),
        include_str!("db.rs"),
        include_str!("sql.rs"),
        include_str!("catalog.rs"),
        include_str!("../../native/src/postgres.rs"),
        include_str!("../../native/src/mysql.rs"),
    ] {
        h.part(source.as_bytes());
    }
    h.finish()
});

// ---------------------------------------------------------------------------------------------
// Turning it off, and what it did (for the tests).

thread_local! {
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
    static STATS: Cell<Stats> = const { Cell::new(Stats { replayed: 0, lines: 0, copies: 0, saved: 0 }) };
}

/// What the cache did on this thread since the last `take_stats`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Steps run in shadows.
    pub replayed: usize,
    /// Catalogs answered from the cache's lines, with no shadow read.
    pub lines: usize,
    /// Shadows started from a cached database.
    pub copies: usize,
    /// Databases saved as caches.
    pub saved: usize,
}

pub fn take_stats() -> Stats {
    STATS.replace(Stats::default())
}

pub(crate) fn count(f: impl FnOnce(&mut Stats)) {
    let mut s = STATS.get();
    f(&mut s);
    STATS.set(s);
}

/// Turns the cache on or off for runs on this thread, whatever `REYLO_NO_CACHE` says; `None` goes
/// back to the variable. For tests, which run on threads of their own.
pub fn set_enabled(on: Option<bool>) {
    OVERRIDE.set(on);
}

pub fn enabled() -> bool {
    OVERRIDE.get().unwrap_or_else(|| !matches!(std::env::var("REYLO_NO_CACHE").as_deref(), Ok(v) if !v.is_empty() && v != "0"))
}

// ---------------------------------------------------------------------------------------------
// The cache of one run.

/// Calls that read the session's sequence or random state, which a migration's predecessors in the
/// same session left: a shadow copied from a cache starts a new session, without it.
static SESSION_STATE: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        Regex::new(r"(?i)\b(currval|lastval|setseed)\b").expect("a valid pattern"),
        Regex::new(r"(?i)\b(last_insert_id|found_rows|row_count)\b").expect("a valid pattern"),
        Regex::new(r"(?i)\b(last_insert_rowid|changes|total_changes)\s*\(").expect("a valid pattern"),
    ]
});

pub(crate) struct Cache {
    d: Dialect,
    base: Hash,
    /// The target, as eviction counts it: a digest of its URL without the user and password, or of
    /// its file.
    target: String,
    /// `.reylo/migrate/` in the project.
    dir: PathBuf,
    /// Whether a shadow may start from a cached database: never on MySQL, which keeps none.
    pub(crate) copies: bool,
    /// The database caches this run started from or made, which eviction keeps.
    used: Vec<String>,
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

impl Cache {
    /// The cache for shadows made on `server` for the target at `url`, or None where it is off or
    /// what decides a replay cannot all be read. `texts` are every step's SQL the run may replay.
    pub(crate) async fn open(server: &mut Server, url: &str, root: &Path, texts: &[&str]) -> Option<Cache> {
        if !enabled() {
            return None;
        }
        let d = server.dialect();
        let mut h = Sha256::new();
        h.part(b"reylo replay cache 1").part(&*CODE).part(d.name().as_bytes());
        let state = server_state(server).await.ok()?;
        h.part(state.as_bytes());
        let target = match d {
            Dialect::Sqlite => crate::db::sqlite_path(url, root),
            _ => crate::redact(url),
        };
        let copies = d != Dialect::Mysql && !texts.iter().any(|t| SESSION_STATE.iter().any(|r| r.is_match(t)));
        Some(Cache { d, base: h.finish(), target: hex(&Sha256::new().part(target.as_bytes()).finish()[..8]), dir: root.join(".reylo").join("migrate"), copies, used: vec![] })
    }

    pub(crate) fn base(&self) -> Hash {
        self.base
    }

    /// The key of a prefix one step longer than `prev`'s.
    pub(crate) fn step(prev: &Hash, schema: bool, label: &str, sql: &str) -> Hash {
        Sha256::new().part(prev).part(if schema { b"script" } else { b"migration" }).part(label.as_bytes()).part(sql.as_bytes()).finish()
    }

    fn lines_file(&self, key: &Hash, comments: bool) -> PathBuf {
        let k = Sha256::new().part(key).part(b"lines").part(&[comments as u8]).finish();
        self.dir.join("lines").join(format!("{}.json", hex(&k[..20])))
    }

    /// The catalog's lines at the prefix `key`, where a run kept them.
    pub(crate) fn read_lines(&self, key: &Hash, comments: bool) -> Option<Vec<String>> {
        let file = self.lines_file(key, comments);
        let text = std::fs::read_to_string(&file).ok()?;
        let lines: Vec<String> = serde_json::from_str(&text).ok()?;
        touch(&file);
        Some(lines)
    }

    pub(crate) fn write_lines(&self, key: &Hash, comments: bool, lines: &[String]) {
        let file = self.lines_file(key, comments);
        if let Ok(text) = serde_json::to_string(lines) {
            write_atomically(&self.dir, &file, text.as_bytes());
        }
    }

    fn pg_name(key: &Hash) -> String {
        format!("{CACHE_PREFIX}{}", hex(&key[..20]))
    }

    fn sqlite_file(&self, key: &Hash) -> PathBuf {
        self.dir.join("sqlite").join(format!("{}.db", hex(&key[..20])))
    }

    /// Which of `keys` have a cached database: the longest first.
    pub(crate) async fn databases(&self, server: &mut Server, keys: &[Hash]) -> Result<Vec<usize>, Error> {
        if !self.copies || keys.is_empty() {
            return Ok(vec![]);
        }
        let mut found: Vec<usize> = match (self.d, server) {
            (Dialect::Postgres, Server::Postgres { admin, .. }) => {
                let names: Vec<String> = keys.iter().map(Cache::pg_name).collect();
                let list = names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ");
                let have: std::collections::HashSet<String> = pg_rows(admin, &format!("select datname from pg_database where datname in ({list})")).await?.iter().map(|r| r.text("datname").to_string()).collect();
                (0..keys.len()).filter(|&i| have.contains(&names[i])).collect()
            }
            (Dialect::Sqlite, _) => (0..keys.len()).filter(|&i| self.sqlite_file(&keys[i]).exists()).collect(),
            _ => vec![],
        };
        found.reverse();
        Ok(found)
    }

    /// A shadow that starts as the cached database `key`, or None where it cannot be had (dropped
    /// since, or held by a run that writes it).
    pub(crate) async fn copy_of(&mut self, server: &mut Server, key: &Hash) -> Option<Shadow> {
        match self.d {
            Dialect::Postgres => {
                let name = Cache::pg_name(key);
                let Server::Postgres { admin, .. } = server else { return None };
                let held = pg_rows(admin, &format!("select pg_try_advisory_lock_shared(hashtextextended('{name}', 0)) as ok")).await.ok()?.first().is_some_and(|r| r.flag("ok"));
                if !held {
                    return None;
                }
                let shadow = server.pg_shadow(Some(&name)).await;
                if let Server::Postgres { admin, .. } = server {
                    let _ = admin.batch_execute(&format!("select pg_advisory_unlock_shared(hashtextextended('{name}', 0))")).await;
                }
                let shadow = shadow.ok()?;
                self.used.push(name);
                Some(shadow)
            }
            Dialect::Sqlite => {
                let file = self.sqlite_file(key);
                let from = rusqlite::Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
                let to = rusqlite::Connection::open_in_memory().ok()?;
                sqlite_backup(&from, &to).ok()?;
                touch(&file);
                Some(Shadow { db: Db::Sqlite(to), name: ":memory:".into() })
            }
            Dialect::Mysql => None,
        }
    }

    /// Keeps the shadow, whose session is as it opened, as the cache of the prefix `key`, and
    /// answers it again to go on with (on Postgres a new session on it, since a database with a
    /// session cannot be copied). A cache another run is writing, or one that exists, is left alone.
    pub(crate) async fn save_copy(&mut self, server: &mut Server, shadow: Shadow, key: &Hash) -> Result<Shadow, Error> {
        match self.d {
            Dialect::Postgres => {
                let name = Cache::pg_name(key);
                let Shadow { db, name: shadow_name } = shadow;
                let Server::Postgres { admin, url, .. } = server else { return Err(Error("not a Postgres server".into())) };
                if pg_rows(admin, &format!("select pg_try_advisory_lock(hashtextextended('{name}', 0)) as ok")).await?.first().is_some_and(|r| r.flag("ok")) {
                    pg_close(admin, db, &shadow_name).await;
                    let made = admin.batch_execute(&format!("create database \"{name}\" template \"{shadow_name}\"")).await;
                    if made.is_ok() {
                        let _ = admin.batch_execute(&format!("comment on database \"{name}\" is {}", literal(&meta(&[(self.target.clone(), now_ms())])))).await;
                        self.used.push(name.clone());
                        count(|s| s.saved += 1);
                    }
                    admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', 0))")).await.map_err(pg_error)?;
                    let db = Db::Postgres(crate::db::open_postgres(&with_database(url, &shadow_name)).await?);
                    return Ok(Shadow { db, name: shadow_name });
                }
                Ok(Shadow { db, name: shadow_name })
            }
            Dialect::Sqlite => {
                self.save_sqlite(&shadow, key);
                Ok(shadow)
            }
            Dialect::Mysql => Ok(shadow),
        }
    }

    /// Keeps a shadow that is done with as the cache of the prefix `key`, by renaming it; answers it
    /// back, to be dropped, where another run is writing that cache. One that another run kept first
    /// is dropped here.
    pub(crate) async fn save(&mut self, server: &mut Server, shadow: Shadow, key: &Hash) -> Option<Shadow> {
        match self.d {
            Dialect::Postgres => {
                let name = Cache::pg_name(key);
                let Shadow { db, name: shadow_name } = shadow;
                let Server::Postgres { admin, .. } = server else { return Some(Shadow { db, name: shadow_name }) };
                let free = pg_rows(admin, &format!("select pg_try_advisory_lock(hashtextextended('{name}', 0)) as ok")).await.ok().and_then(|r| r.first().map(|r| r.flag("ok"))).unwrap_or(false);
                if !free {
                    return Some(Shadow { db, name: shadow_name });
                }
                pg_close(admin, db, &shadow_name).await;
                let renamed = admin.batch_execute(&format!("alter database \"{shadow_name}\" rename to \"{name}\"")).await;
                if renamed.is_ok() {
                    let _ = admin.batch_execute(&format!("comment on database \"{name}\" is {}", literal(&meta(&[(self.target.clone(), now_ms())])))).await;
                    let _ = admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{shadow_name}', 0))")).await;
                    self.used.push(name.clone());
                    count(|s| s.saved += 1);
                }
                let _ = admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', 0))")).await;
                if renamed.is_err() {
                    // Not kept (another run kept the same prefix first): dropped as a shadow is.
                    let _ = admin.batch_execute(&format!("drop database if exists \"{shadow_name}\" with (force)")).await;
                    let _ = admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{shadow_name}', 0))")).await;
                }
                None
            }
            Dialect::Sqlite => {
                self.save_sqlite(&shadow, key);
                Some(shadow)
            }
            Dialect::Mysql => Some(shadow),
        }
    }

    fn save_sqlite(&mut self, shadow: &Shadow, key: &Hash) {
        let Db::Sqlite(conn) = &shadow.db else { return };
        let file = self.sqlite_file(key);
        if file.exists() {
            return;
        }
        let Some(dir) = file.parent() else { return };
        ensure_dir(&self.dir, dir);
        let tmp = dir.join(format!("{}.{}.tmp", random_name(), std::process::id()));
        let written = rusqlite::Connection::open(&tmp).ok().and_then(|to| sqlite_backup(conn, &to).ok());
        if written.is_some() && std::fs::rename(&tmp, &file).is_ok() {
            count(|s| s.saved += 1);
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    /// At the end of a run: the caches it used are marked used by its target, and each target keeps
    /// its newest few; the project keeps its newest lines.
    pub(crate) async fn finish(self, server: &mut Server) {
        prune(&self.dir.join("lines"), KEEP_LINES);
        if self.d == Dialect::Sqlite {
            prune(&self.dir.join("sqlite"), KEEP_SQLITE);
        }
        let Server::Postgres { admin, .. } = server else { return };
        let _ = evict(admin, &self.target, &self.used).await;
    }
}

/// Everything about the server a replay's outcome turns on, beside the steps.
async fn server_state(server: &mut Server) -> Result<String, Error> {
    match server {
        Server::Postgres { admin, create, settings, privileges, copy_privileges, .. } => {
            // The server and its settings as a session sees them, the roles a grant may name, the
            // extensions a migration may create, and template1, which a new shadow is a copy of: a
            // change to it moves its counters of rows written.
            let rows = pg_rows(
                admin,
                "select version() as version, current_user as role,
                  (select string_agg(name || '=' || setting, E'\\n' order by name) from pg_settings) as settings,
                  (select string_agg(concat_ws(':', rolname, rolsuper, rolinherit, rolcreaterole, rolcreatedb, rolcanlogin, rolreplication, rolbypassrls), E'\\n' order by rolname) from pg_roles) as roles,
                  (select string_agg(format('%s>%s:%s', roleid::regrole, member::regrole, admin_option), E'\\n' order by roleid::regrole::text, member::regrole::text) from pg_auth_members) as members,
                  (select string_agg(name || ' ' || coalesce(default_version, ''), E'\\n' order by name) from pg_available_extensions) as extensions,
                  (select concat_ws(':', tup_inserted, tup_updated, tup_deleted, stats_reset) from pg_stat_database where datname = 'template1') as template",
            )
            .await?;
            let r = rows.first().ok_or_else(|| Error("no answer".into()))?;
            let mut out = ["version", "role", "settings", "roles", "members", "extensions", "template"].map(|k| r.text(k).to_string()).join("\n\x1f");
            out.push_str(&format!("\n\x1fcreate {create}\n\x1fsettings {}\n\x1fprivileges {copy_privileges} {}", settings.join("\n"), privileges.join("\n")));
            Ok(out)
        }
        Server::Mysql { admin, defaults, .. } => {
            use mysql_async::prelude::Queryable;
            let map = |e: mysql_async::Error| Error(e.to_string());
            let head: Option<(String, String, String)> = admin.query_first("select version(), @@server_uuid, current_user()").await.map_err(map)?;
            let globals: Vec<(String, String)> = admin.query("select variable_name, variable_value from performance_schema.global_variables order by variable_name").await.map_err(map)?;
            let users: Vec<(String, String)> = admin.query("select user, host from mysql.user order by user, host").await.map_err(map)?;
            let (v, u, who) = head.unwrap_or_default();
            let globals: Vec<String> = globals.into_iter().filter(|(k, _)| !k.starts_with("gtid_")).map(|(k, v)| format!("{k}={v}")).collect();
            let users: Vec<String> = users.into_iter().map(|(u, h)| format!("{u}@{h}")).collect();
            Ok(format!("{v}\n\x1f{u}\n\x1f{who}\n\x1f{}\n\x1f{}\n\x1f{defaults:?}", globals.join("\n"), users.join("\n")))
        }
        Server::Sqlite => {
            let conn = rusqlite::Connection::open_in_memory().map_err(|e| Error(e.to_string()))?;
            let mut statement = conn.prepare("pragma compile_options").map_err(|e| Error(e.to_string()))?;
            let options: Vec<String> = statement.query_map([], |r| r.get(0)).map_err(|e| Error(e.to_string()))?.filter_map(|r| r.ok()).collect();
            Ok(format!("{}\n\x1f{}", rusqlite::version(), options.join("\n")))
        }
    }
}

/// The session of a shadow as it bears on what its next steps do and on what its catalog reads:
/// its settings, its role, its temporary objects, prepared statements, cursors, channels and locks,
/// and its database's own settings, owner, privileges and comment, which a copy does not take.
/// Two snapshots of one session are equal when nothing a step left in it lasts.
pub(crate) async fn snapshot(db: &mut Db) -> Result<String, Error> {
    match db.dialect() {
        Dialect::Postgres => {
            let rows = db
                .rows(
                    "select concat_ws(E'\\n',
                      (select string_agg(name || '=' || setting, ',' order by name) from pg_settings),
                      current_user, session_user,
                      (select count(*) from pg_class where relnamespace = pg_my_temp_schema()),
                      (select count(*) from pg_proc where pronamespace = pg_my_temp_schema()),
                      (select count(*) from pg_type where typnamespace = pg_my_temp_schema()),
                      (select count(*) from pg_prepared_statements), (select count(*) from pg_cursors), (select count(*) from pg_listening_channels()),
                      (select count(*) from pg_locks where pid = pg_backend_pid() and locktype = 'advisory'),
                      (select concat_ws(',', datdba, datacl, datconnlimit, datallowconn, datistemplate) from pg_database where datname = current_database()),
                      (select string_agg(setrole || ':' || array_to_string(setconfig, ','), ';' order by setrole) from pg_db_role_setting where setdatabase = (select oid from pg_database where datname = current_database())),
                      (select description from pg_shdescription where classoid = 'pg_database'::regclass and objoid = (select oid from pg_database where datname = current_database()))) as s",
                )
                .await?;
            Ok(rows.first().map(|r| r.text("s").to_string()).unwrap_or_default())
        }
        Dialect::Mysql => {
            // What changes with every statement or row is left out; a text that reads it is never
            // replayed from a copy.
            const MOVING: &[&str] = &["timestamp", "statement_id", "identity", "last_insert_id", "insert_id", "rand_seed1", "rand_seed2"];
            let vars = db.rows("select variable_name as k, variable_value as v from performance_schema.session_variables").await?;
            let mut out: Vec<String> = vars.iter().filter(|r| !MOVING.contains(&r.text("k"))).map(|r| format!("{}={}", r.text("k"), r.text("v"))).collect();
            out.sort();
            let rest = db.rows("select (select count(*) from performance_schema.user_variables_by_thread where thread_id = ps_current_thread_id()) as u, current_user() as who, database() as db").await?;
            if let Some(r) = rest.first() {
                out.push(format!("{} {} {}", r.text("u"), r.text("who"), r.text("db")));
            }
            Ok(out.join("\n"))
        }
        Dialect::Sqlite => {
            // `foreign_keys` is set by every migration's run itself, before it and after it.
            const PRAGMAS: &[&str] = &[
                "recursive_triggers",
                "legacy_alter_table",
                "trusted_schema",
                "automatic_index",
                "defer_foreign_keys",
                "ignore_check_constraints",
                "query_only",
                "reverse_unordered_selects",
                "writable_schema",
                "cell_size_check",
                "journal_mode",
                "synchronous",
                "temp_store",
                "cache_size",
                "secure_delete",
                "encoding",
                "busy_timeout",
            ];
            let mut out = Vec::new();
            for p in PRAGMAS {
                out.push(format!("{p}={}", db.rows(&format!("pragma {p}")).await?.first().and_then(|r| r.first().map(String::from)).unwrap_or_default()));
            }
            out.push(format!("temp={}", db.rows("select count(*) as n from temp.sqlite_schema").await?.first().map(|r| r.text("n").to_string()).unwrap_or_default()));
            out.push(format!("databases={}", db.rows("select count(*) as n from pragma_database_list").await?.first().map(|r| r.text("n").to_string()).unwrap_or_default()));
            Ok(out.join("\n"))
        }
    }
}

/// Copies the main database of `from` over that of `to`, page by page, as SQLite's backup API does.
fn sqlite_backup(from: &rusqlite::Connection, to: &rusqlite::Connection) -> Result<(), Error> {
    use rusqlite::ffi;
    // SAFETY: both handles are open connections that outlive the backup, which is finished before
    // this returns; neither is used by another thread meanwhile.
    unsafe {
        let backup = ffi::sqlite3_backup_init(to.handle(), c"main".as_ptr(), from.handle(), c"main".as_ptr());
        if backup.is_null() {
            return Err(Error("the SQLite backup did not start".into()));
        }
        let step = ffi::sqlite3_backup_step(backup, -1);
        let finished = ffi::sqlite3_backup_finish(backup);
        if step != ffi::SQLITE_DONE || finished != ffi::SQLITE_OK {
            return Err(Error(format!("the SQLite backup failed ({step}, {finished})")));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The project's files: written to a file of their own and renamed into place, so that a reader
// sees a whole entry or none; a directory that cannot be written is no error, since the cache is
// only for speed.

fn ensure_dir(root: &Path, dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    // `.reylo/.gitignore` keeps the directory out of git, as the type check's cache beside it does.
    if let Some(reylo) = root.parent() {
        let ignore = reylo.join(".gitignore");
        if !ignore.exists() {
            let _ = std::fs::write(ignore, "# reylo's cache: written by the type check, the build steps, reylo generate and reylo migrate; never committed.\n*\n");
        }
    }
}

fn write_atomically(root: &Path, file: &Path, bytes: &[u8]) {
    let Some(dir) = file.parent() else { return };
    ensure_dir(root, dir);
    let tmp = dir.join(format!("{}.{}.tmp", random_name(), std::process::id()));
    if std::fs::write(&tmp, bytes).is_err() || std::fs::rename(&tmp, file).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Marks a file used, for `prune`.
fn touch(file: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(file) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
}

/// Keeps the `keep` newest entries of `dir` and none a week old; leftovers of a write cut off an
/// hour ago go too.
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let now = std::time::SystemTime::now();
    let age = |m: &std::fs::Metadata| m.modified().ok().and_then(|t| now.duration_since(t).ok()).map(|d| d.as_millis()).unwrap_or(0);
    let mut files: Vec<(u128, PathBuf)> = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        let Ok(m) = e.metadata() else { continue };
        if path.extension().is_some_and(|x| x == "tmp") {
            if age(&m) > 3600 * 1000 {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        files.push((age(&m), path));
    }
    files.sort();
    for (k, (age, path)) in files.iter().enumerate() {
        if k >= keep || *age > STALE_MS {
            let _ = std::fs::remove_file(path);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Postgres's database caches. Each carries, as its comment, the targets that used it and when; the
// session lock named after it is held shared by a run copying it, and alone by one that writes or
// drops it, so neither happens under the other. A second lock guards its comment.

fn meta(targets: &[(String, u128)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = targets.iter().map(|(t, at)| (t.clone(), serde_json::Value::from(*at as u64))).collect();
    serde_json::json!({ "reylo": "replay cache", "targets": map }).to_string()
}

fn parse_meta(text: Option<&str>) -> Option<BTreeMap<String, u128>> {
    let v: serde_json::Value = serde_json::from_str(text?).ok()?;
    if v["reylo"] != "replay cache" {
        return None;
    }
    Some(v["targets"].as_object()?.iter().filter_map(|(k, at)| Some((k.clone(), at.as_u64()? as u128))).collect())
}

async fn try_lock(admin: &tokio_postgres::Client, name: &str, which: u8) -> bool {
    pg_rows(admin, &format!("select pg_try_advisory_lock(hashtextextended('{name}', {which})) as ok")).await.ok().and_then(|r| r.first().map(|r| r.flag("ok"))).unwrap_or(false)
}

async fn unlock(admin: &tokio_postgres::Client, name: &str, which: u8) {
    let _ = admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', {which}))")).await;
}

async fn caches(admin: &tokio_postgres::Client) -> Result<Vec<(String, Option<BTreeMap<String, u128>>)>, Error> {
    let rows = pg_rows(admin, &format!(r"select datname, shobj_description(oid, 'pg_database') as meta from pg_database where datname like '{}%'", CACHE_PREFIX.replace('_', r"\_"))).await?;
    Ok(rows.iter().map(|r| (r.text("datname").to_string(), parse_meta(r.get("meta")))).collect())
}

async fn drop_cache(admin: &tokio_postgres::Client, name: &str) -> bool {
    if !try_lock(admin, name, 0).await {
        return false;
    }
    let dropped = admin.batch_execute(&format!("drop database if exists \"{name}\" with (force)")).await.is_ok();
    unlock(admin, name, 0).await;
    dropped
}

async fn write_meta(admin: &tokio_postgres::Client, name: &str, targets: &BTreeMap<String, u128>) {
    if try_lock(admin, name, 1).await {
        let list: Vec<(String, u128)> = targets.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let _ = admin.batch_execute(&format!("comment on database \"{name}\" is {}", literal(&meta(&list)))).await;
        unlock(admin, name, 1).await;
    }
}

/// Marks `used` as used by `target` now, then keeps the target's newest `KEEP` and the server's
/// newest `KEEP_ON_SERVER`; a cache no target keeps is dropped.
async fn evict(admin: &tokio_postgres::Client, target: &str, used: &[String]) -> Result<(), Error> {
    let now = now_ms();
    let mut all = caches(admin).await?;
    for (name, targets) in all.iter_mut() {
        if used.contains(name) {
            let mut map = targets.clone().unwrap_or_default();
            map.insert(target.to_string(), now);
            write_meta(admin, name, &map).await;
            *targets = Some(map);
        }
    }
    let mut mine: Vec<(u128, String)> = all.iter().filter_map(|(n, t)| Some((*t.as_ref()?.get(target)?, n.clone()))).collect();
    mine.sort_by(|a, b| b.cmp(a));
    for (_, name) in mine.iter().skip(KEEP) {
        let Some((_, Some(targets))) = all.iter_mut().find(|(n, _)| n == name) else { continue };
        targets.remove(target);
        if targets.is_empty() {
            drop_cache(admin, name).await;
        } else {
            write_meta(admin, name, targets).await;
        }
    }
    let mut kept: Vec<(u128, String)> = all.iter().filter_map(|(n, t)| Some((t.as_ref()?.values().max().copied()?, n.clone()))).collect();
    kept.sort_by(|a, b| b.cmp(a));
    for (_, name) in kept.iter().skip(KEEP_ON_SERVER) {
        drop_cache(admin, name).await;
    }
    Ok(())
}

/// Drops the caches no run will use: one whose comment is missing (a run cut off between making it
/// and marking it) or that no target used for a week.
pub(crate) async fn sweep(server: &mut Server) -> Result<(), Error> {
    let Server::Postgres { admin, .. } = server else { return Ok(()) };
    let now = now_ms();
    for (name, targets) in caches(admin).await? {
        let stale = match &targets {
            None => true,
            Some(t) => t.values().all(|at| now.saturating_sub(*at) > STALE_MS),
        };
        if stale {
            drop_cache(admin, &name).await;
        }
    }
    Ok(())
}

/// Forgets `url`'s target: its marks go, and every cache no other target keeps is dropped. For a
/// test, or a target that is dropped.
pub async fn forget(url: &str) -> Result<(), Error> {
    if Dialect::of_url(url)? != Dialect::Postgres {
        return Ok(());
    }
    let admin = crate::db::open_postgres(&with_database(url, "postgres")).await?;
    let target = hex(&Sha256::new().part(crate::redact(url).as_bytes()).finish()[..8]);
    for (name, targets) in caches(&admin).await? {
        let Some(mut targets) = targets else { continue };
        if targets.remove(&target).is_none() {
            continue;
        }
        if targets.is_empty() {
            drop_cache(&admin, &name).await;
        } else {
            write_meta(&admin, &name, &targets).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(hex(&Sha256::new().finish()), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&Sha256::new().update(b"abc").finish()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(hex(&Sha256::new().update(long).finish()), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
        let mut split = Sha256::new();
        for chunk in long.chunks(7) {
            split.update(chunk);
        }
        assert_eq!(split.finish(), Sha256::new().update(long).finish());
        let million = vec![b'a'; 1_000_000];
        assert_eq!(hex(&Sha256::new().update(&million).finish()), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn a_session_state_call_is_found_in_any_case() {
        assert!(SESSION_STATE.iter().any(|r| r.is_match("insert into t values (currval('s'))")));
        assert!(SESSION_STATE.iter().any(|r| r.is_match("select LAST_INSERT_ROWID ()")));
        // A table named `changes` is no call, unless a parenthesis follows it, which only costs a copy.
        assert!(!SESSION_STATE.iter().any(|r| r.is_match("alter table changes add column note text")));
    }
}
