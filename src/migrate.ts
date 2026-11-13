// What an import of the project file gives a Cloudflare Durable Object: `migrate`, which runs the
// project's migrations on the object's own SQLite database. Nothing outside the object can reach
// that database, so `reylo migrate deploy` can't run there: each object runs what it hasn't run yet
// when it starts, from the migrations the bundler plugin put in the bundle (src/project-module.ts),
// in the order `reylo migrate deploy` runs them, and records each in the history table `reylo
// migrate` keeps on SQLite (`history_ddl` in engine/migrate/src/commands.rs), so that both read
// the same rows. Before it runs any, it holds the history against the bundle as a deploy does with
// no shadow database, and refuses a history the bundle doesn't continue.

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
 * ran. A migration that fails rolls back with its row, and the error names it.
 *
 * Before it runs any, it refuses a history the bundle doesn't continue, naming the migration: one
 * the object ran that the bundle lacks, one whose text in the bundle isn't the text it ran, and
 * migrations it ran in another order than the bundle's.
 */
export function runMigrations(storage: MigrationStorage, list: readonly Migration[]): string[] {
  const { sql } = storage;
  sql.exec(`create table if not exists ${HISTORY} (name text primary key, sql text not null, applied_at text, fingerprint text, catalog text)`);
  // In the order they ran, as reylo migrate reads its history (`read_history`).
  const ran = sql.exec(`select name, sql from ${HISTORY} order by applied_at is null, applied_at, rowid, case when name = 'init.sql' then '' else name end`).toArray() as { name: string; sql: string }[];
  const bundled = new Map(list.map((m) => [m.name, m]));
  ran.forEach((row, i) => {
    const m = bundled.get(row.name);
    if (!m) throw new Error(`reylo: this Durable Object ran the migration ${row.name}, which this build doesn't have: deploy a build whose migrations include it`);
    if (m.sql.trim() !== row.sql.trim()) throw new Error(`reylo: the migration ${row.name} isn't the text this Durable Object ran: put the change in a new migration instead of editing one an object may have run`);
    if (list[i]?.name !== row.name) throw new Error(`reylo: this Durable Object ran ${row.name} where this build's order has ${list[i]?.name ?? "nothing"} first: migrations run in the lock's order, so add a new one after those that ran`);
  });
  const done = new Set(ran.map((r) => r.name));
  const applied: string[] = [];
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
    applied.push(m.name);
  }
  return applied;
}

/**
 * What an import of the project file calls, with the project's migrations, in the module the
 * bundler plugin writes for it: `import { migrate } from "./.reylo.yaml"` then gives `migrate`.
 */
export function migrations(list: readonly Migration[]): { migrate(storage: MigrationStorage): string[] } {
  return { migrate: (storage) => runMigrations(storage, list) };
}
