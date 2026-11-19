/* libSQL's engine, for the oracle's libSQL cases (src/libsql.rs): the amalgamation of Turso's
 * `libsql-ffi` crate, which the prepare step writes into engine/migrate/.libsql/, compiled into this
 * one translation unit with its API private, since the oracle links rusqlite's SQLite too. Only
 * these `reylo_oracle_libsql_` functions are exported. */
#define SQLITE_API static
#define SQLITE_EXTERN
#include "sqlite3-private.c"

int reylo_oracle_libsql_open(const char *path, sqlite3 **db) {
  return sqlite3_open_v2(path, db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, 0);
}
int reylo_oracle_libsql_close(sqlite3 *db) { return sqlite3_close_v2(db); }
const char *reylo_oracle_libsql_errmsg(sqlite3 *db) { return sqlite3_errmsg(db); }
int reylo_oracle_libsql_extended_errcode(sqlite3 *db) { return sqlite3_extended_errcode(db); }
const char *reylo_oracle_libsql_version(void) { return sqlite3_libversion(); }
int reylo_oracle_libsql_autocommit(sqlite3 *db) { return sqlite3_get_autocommit(db); }
int reylo_oracle_libsql_changes(sqlite3 *db) { return sqlite3_changes(db); }
void reylo_oracle_libsql_progress(sqlite3 *db, int n, int (*callback)(void *), void *arg) { sqlite3_progress_handler(db, n, callback, arg); }

/* Prepares the first statement of `sql`, `*tail` where the next one starts; a statement of nothing
 * (a comment, the end) is a null `*stmt`. */
int reylo_oracle_libsql_prepare(sqlite3 *db, const char *sql, sqlite3_stmt **stmt, const char **tail) { return sqlite3_prepare_v2(db, sql, -1, stmt, tail); }
int reylo_oracle_libsql_finalize(sqlite3_stmt *stmt) { return sqlite3_finalize(stmt); }
int reylo_oracle_libsql_step(sqlite3_stmt *stmt) { return sqlite3_step(stmt); }
int reylo_oracle_libsql_bind_null(sqlite3_stmt *stmt, int i) { return sqlite3_bind_null(stmt, i); }
int reylo_oracle_libsql_bind_int64(sqlite3_stmt *stmt, int i, long long v) { return sqlite3_bind_int64(stmt, i, v); }
int reylo_oracle_libsql_bind_double(sqlite3_stmt *stmt, int i, double v) { return sqlite3_bind_double(stmt, i, v); }
int reylo_oracle_libsql_bind_text(sqlite3_stmt *stmt, int i, const char *v, int n) { return sqlite3_bind_text(stmt, i, v, n, SQLITE_TRANSIENT); }
int reylo_oracle_libsql_bind_blob(sqlite3_stmt *stmt, int i, const void *v, int n) { return sqlite3_bind_blob(stmt, i, v, n, SQLITE_TRANSIENT); }
int reylo_oracle_libsql_column_count(sqlite3_stmt *stmt) { return sqlite3_column_count(stmt); }
const char *reylo_oracle_libsql_column_name(sqlite3_stmt *stmt, int i) { return sqlite3_column_name(stmt, i); }
int reylo_oracle_libsql_column_type(sqlite3_stmt *stmt, int i) { return sqlite3_column_type(stmt, i); }
long long reylo_oracle_libsql_column_int64(sqlite3_stmt *stmt, int i) { return sqlite3_column_int64(stmt, i); }
double reylo_oracle_libsql_column_double(sqlite3_stmt *stmt, int i) { return sqlite3_column_double(stmt, i); }
const void *reylo_oracle_libsql_column_blob(sqlite3_stmt *stmt, int i) { return sqlite3_column_blob(stmt, i); }
const unsigned char *reylo_oracle_libsql_column_text(sqlite3_stmt *stmt, int i) { return sqlite3_column_text(stmt, i); }
int reylo_oracle_libsql_column_bytes(sqlite3_stmt *stmt, int i) { return sqlite3_column_bytes(stmt, i); }
