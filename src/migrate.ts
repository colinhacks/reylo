// `migrate` from reylo's root: `reylo migrate deploy`, with two hosts. On a server, `await migrate()`
// runs the binary's deploy, as the command line does (src/migrate-deploy.ts, through the build-time
// half). On a Cloudflare Durable Object, whose SQLite database nothing outside the object can
// reach, `migrate(ctx.storage)` runs the deploy's last step here: each migration the object hasn't
// run yet, from those Reylo's bundler plugin put in the bundle (src/embedded-migrations.ts), in the
// order the deploy runs them, recorded in the history table `reylo migrate` keeps on SQLite
// (`history_ddl` in engine/migrate/src/commands.rs), so that both read the same rows. Before it runs
// any, it holds the history against the bundle as a deploy does with no shadow database, and refuses
// a history the bundle doesn't continue.
import { buildTime, type MigrateOptions } from "./build-time.ts";
import { embedded } from "./embedded-migrations.ts";
import { sameMigration } from "./migration-text.ts";

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
    // As the lock compares them: comments, spacing and line endings aside (src/migration-text.ts).
    if (!sameMigration(m.sql, row.sql)) throw new Error(`reylo: the migration ${row.name} isn't the text this Durable Object ran: put the change in a new migration instead of editing one an object may have run`);
    if (list[i]?.name !== row.name) throw new Error(`reylo: this Durable Object ran ${row.name} where this build's order has ${list[i]?.name ?? "nothing"} first: migrations run in the lock's order, so add a new one after those that ran`);
  });
  const done = new Set(ran.map((r) => r.name));
  const applied: string[] = [];
  for (const m of list) {
    if (done.has(m.name)) continue;
    try {
      storage.transactionSync(() => {
        sql.exec(m.sql);
        // As reylo migrate checks a SQLite migration's foreign keys before it commits (`run` in
        // engine/migrate/src/commands.rs): a deferred one it broke fails here, rolling back the
        // migration alone, rather than at the object's commit, which resets the object.
        const broken = sql.exec("pragma foreign_key_check").toArray().length;
        if (broken) throw new Error(`${broken} rows break a foreign key after this migration`);
        // The check read every key and found each whole, so what SQLite still counts as deferred
        // violations is none: a table's drop counts its children's rows, and making it again does
        // not uncount them. Left counted, they fail the object's commit, which resets the object,
        // and with the migration's row gone too, every start runs it and resets again.
        sql.exec("pragma defer_foreign_keys = off");
        sql.exec(`insert into ${HISTORY} (name, sql, applied_at) values (?, ?, strftime('%Y-%m-%d %H:%M:%f', 'now'))`, m.name, m.sql);
      });
    } catch (e) {
      throw new Error(`reylo: the migration ${m.name} failed on this Durable Object, and rolled back: ${(e as Error).message}`, { cause: e });
    }
    applied.push(m.name);
  }
  return applied;
}

const isStorage = (v: unknown): v is MigrationStorage => typeof v === "object" && v !== null && typeof (v as MigrationStorage).transactionSync === "function" && typeof (v as MigrationStorage).sql?.exec === "function";

/**
 * Runs the project's migrations on a SQLite-backed Durable Object's storage, `ctx.storage`: each the
 * object hasn't run yet, in the order `reylo migrate deploy` runs them, each in a transaction with its
 * row in `__reylo_migrations`. Returns the names it ran. Call it in the object's constructor.
 *
 * Reylo's Vite or esbuild plugin puts the locked migrations in the bundle. Before it runs any, it
 * throws, running nothing, where the object ran a migration the bundle lacks, has edited or orders
 * differently.
 */
export function migrate(storage: MigrationStorage): string[];
/**
 * Runs `reylo migrate deploy` on the project's database, or the one `url` names, as the command line
 * does, and resolves to the names of the migrations it ran. Rejects with the deploy's problems where
 * it refuses, as on a draft or a database changed by hand. It runs on Node and Bun, from the
 * project's directory.
 */
export function migrate(options?: MigrateOptions): Promise<string[]>;
export function migrate(arg?: MigrationStorage | MigrateOptions): string[] | Promise<string[]> {
  // A promise that rejects, never a throw, where the build-time half can't be loaded.
  if (!isStorage(arg)) return Promise.resolve(arg ?? {}).then((options) => buildTime().migrate(options));
  if (embedded === undefined) throw new Error("reylo: this bundle holds no migrations for migrate: build it with reylo/vite or reylo/esbuild, in a project whose migrations are in reylo/migrations");
  if ("error" in embedded) throw new Error(`reylo: ${embedded.error}`);
  return runMigrations(arg, embedded as readonly Migration[]);
}
