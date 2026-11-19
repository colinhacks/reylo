/* libSQL's engine, for the migrations of a project whose SQL is libSQL's own (a vector index, an
 * `alter table … alter column`): the amalgamation of Turso's `libsql-ffi` crate, which the prepare
 * step writes into `.libsql/`, compiled into this one translation unit with its API private, as the
 * analysis engine keeps its SQLite (engine/sqlite/build.rs). The binary links other copies of SQLite,
 * rusqlite's among them, so only these `reylo_libsql_` functions are exported, and none of them
 * shadows the other copies' names. */
#define SQLITE_API static
#define SQLITE_EXTERN
#include "sqlite3-private.c"

/* The sqlean extensions Turso Cloud loads into every connection (uuid, crypto, fuzzy, math, stats,
 * text, regexp), as libsql-ffi loads them: `SQLITE_EXTRA_INIT` registers them as an automatic
 * extension when the engine starts. Their sources, compiled apart (build.rs), reach the engine only
 * through the table of routines an extension is handed, which `reylo_libsql_api` holds, never
 * through the private API's names. */
const sqlite3_api_routines *reylo_libsql_api = 0;
#include "crypto/extension.h"
#include "fuzzy/extension.h"
#include "math/extension.h"
#include "stats/extension.h"
#include "text/extension.h"
#include "uuid/extension.h"
#include "regexp/extension.h"

static int reylo_libsql_sqlean(sqlite3 *db, char **err, const sqlite3_api_routines *api) {
  (void)err;
  reylo_libsql_api = api;
  crypto_init(db);
  fuzzy_init(db);
  math_init(db);
  stats_init(db);
  text_init(db);
  uuid_init(db);
  regexp_init(db);
  return SQLITE_OK;
}

int reylo_libsql_extra_init(const char *unused) {
  (void)unused;
  return sqlite3_auto_extension((void (*)(void))reylo_libsql_sqlean);
}

int reylo_libsql_open(const char *path, sqlite3 **db) {
  return sqlite3_open_v2(path, db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_URI, 0);
}
int reylo_libsql_close(sqlite3 *db) { return sqlite3_close_v2(db); }
const char *reylo_libsql_errmsg(sqlite3 *db) { return sqlite3_errmsg(db); }
const char *reylo_libsql_version(void) { return sqlite3_libversion(); }

/* Runs `sql`, one statement or several, stopping at the first that fails. */
int reylo_libsql_exec(sqlite3 *db, const char *sql) { return sqlite3_exec(db, sql, 0, 0, 0); }

int reylo_libsql_prepare(sqlite3 *db, const char *sql, sqlite3_stmt **stmt) { return sqlite3_prepare_v2(db, sql, -1, stmt, 0); }
int reylo_libsql_bind_text(sqlite3_stmt *stmt, int i, const char *text, int n) { return sqlite3_bind_text(stmt, i, text, n, SQLITE_TRANSIENT); }
int reylo_libsql_step(sqlite3_stmt *stmt) { return sqlite3_step(stmt); }
int reylo_libsql_finalize(sqlite3_stmt *stmt) { return sqlite3_finalize(stmt); }
int reylo_libsql_column_count(sqlite3_stmt *stmt) { return sqlite3_column_count(stmt); }
const char *reylo_libsql_column_name(sqlite3_stmt *stmt, int i) { return sqlite3_column_name(stmt, i); }
int reylo_libsql_column_type(sqlite3_stmt *stmt, int i) { return sqlite3_column_type(stmt, i); }
long long reylo_libsql_column_int64(sqlite3_stmt *stmt, int i) { return sqlite3_column_int64(stmt, i); }
double reylo_libsql_column_double(sqlite3_stmt *stmt, int i) { return sqlite3_column_double(stmt, i); }
const void *reylo_libsql_column_blob(sqlite3_stmt *stmt, int i) { return sqlite3_column_blob(stmt, i); }
const unsigned char *reylo_libsql_column_text(sqlite3_stmt *stmt, int i) { return sqlite3_column_text(stmt, i); }
int reylo_libsql_column_bytes(sqlite3_stmt *stmt, int i) { return sqlite3_column_bytes(stmt, i); }

/* Copies the main database of `from` over that of `to`, whole, as SQLite's backup API does. */
int reylo_libsql_backup(sqlite3 *to, sqlite3 *from) {
  sqlite3_backup *backup = sqlite3_backup_init(to, "main", from, "main");
  if (!backup) return sqlite3_errcode(to);
  int step = sqlite3_backup_step(backup, -1);
  int finished = sqlite3_backup_finish(backup);
  return step == SQLITE_DONE ? finished : step;
}
