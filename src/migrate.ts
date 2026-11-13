// What an import of the project file gives a Cloudflare Durable Object: `migrate`, which runs the
// project's migrations on the object's own SQLite database. Nothing outside the object can reach
// that database, so `reylo migrate deploy` can't run there: each object runs what it hasn't run yet
// when it starts, from the migrations the bundler plugin put in the bundle (src/project-module.ts),
// in the order `reylo migrate deploy` runs them, and records each in the history table `reylo
// migrate` keeps on SQLite (`history_ddl` in engine/migrate/src/commands.rs), so that both read
// the same rows.

/** A Durable Object's storage, `ctx.storage`, as far as `migrate` uses it. */
export type MigrationStorage = {
  readonly sql: { exec(query: string, ...bindings: any[]): { toArray(): unknown[] } };
  transactionSync<T>(closure: () => T): T;
};

/** A migration as the bundle carries it: its file's name and its SQL as written. */
export type Migration = { readonly name: string; readonly sql: string };

const HISTORY = "__reylo_migrations";

/**
 * Runs, on a SQLite-backed Durable Object's storage, each of `list` the database hasn't run, in
 * order, each in a transaction together with its row in the history table, and returns the names it
 * ran. A migration that fails rolls back with its row, and the error names it. A migration the
 * history holds that `list` lacks, as after a deploy rolled back past it, is left as it is.
 */
export function runMigrations(storage: MigrationStorage, list: readonly Migration[]): string[] {
  const { sql } = storage;
  sql.exec(`create table if not exists ${HISTORY} (name text primary key, sql text not null, applied_at text, fingerprint text, catalog text)`);
  const done = new Set(sql.exec(`select name from ${HISTORY}`).toArray().map((r) => (r as { name: string }).name));
  const ran: string[] = [];
  for (const m of list) {
    if (done.has(m.name)) continue;
    try {
      storage.transactionSync(() => {
        sql.exec(m.sql);
        sql.exec(`insert into ${HISTORY} (name, sql, applied_at) values (?, ?, strftime('%Y-%m-%d %H:%M:%f', 'now'))`, m.name, m.sql);
      });
    } catch (e) {
      throw new Error(`reylo: the migration ${m.name} failed on this Durable Object, and rolled back: ${(e as Error).message}`, { cause: e });
    }
    ran.push(m.name);
  }
  return ran;
}

/**
 * What an import of the project file calls, with the project's migrations, in the module the
 * bundler plugin writes for it: `import { migrate } from "./.reylo.yaml"` then gives `migrate`.
 */
export function migrations(list: readonly Migration[]): { migrate(storage: MigrationStorage): string[] } {
  return { migrate: (storage) => runMigrations(storage, list) };
}
