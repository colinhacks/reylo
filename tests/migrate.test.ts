import { test } from "node:test";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { migrations, runMigrations, type MigrationStorage } from "../src/migrate.ts";

// A Durable Object's storage over node:sqlite, as far as `migrate` uses it: `sql.exec` runs one or
// more statements, its bindings going to the last, and `transactionSync` rolls back on a throw.
function storage(db = new DatabaseSync(":memory:")): MigrationStorage & { db: DatabaseSync } {
  return {
    db,
    sql: {
      exec(query: string, ...bindings: unknown[]) {
        const statements = query.split(/;\s*(?=\S)/).filter((s) => s.trim());
        const last = statements.pop()!;
        for (const s of statements) db.exec(s);
        const prepared = db.prepare(last);
        const rows = prepared.columns().length ? prepared.all(...(bindings as [])) : (prepared.run(...(bindings as [])), []);
        return { toArray: () => rows };
      },
    },
    transactionSync<T>(closure: () => T): T {
      db.exec("begin");
      try {
        const out = closure();
        db.exec("commit");
        return out;
      } catch (e) {
        db.exec("rollback");
        throw e;
      }
    },
  };
}

const USERS = { name: "20261105120000_users.sql", sql: "create table users (id integer primary key, name text not null);" };
const BIO = { name: "20261106120000_bio.sql", sql: "alter table users add column bio text;" };
const columns = (s: { db: DatabaseSync }) => (s.db.prepare("select name from pragma_table_info('users')").all() as { name: string }[]).map((r) => r.name);

test("migrate runs each migration once, in order, and records it as reylo migrate does", () => {
  const s = storage();
  assert.deepEqual(migrations([USERS]).migrate(s), [USERS.name]);
  s.db.exec("insert into users (name) values ('ada')");
  // The next deploy adds a migration: an object that ran the first runs only the second, and keeps its rows.
  assert.deepEqual(migrations([USERS, BIO]).migrate(s), [BIO.name]);
  assert.deepEqual(columns(s), ["id", "name", "bio"]);
  assert.equal((s.db.prepare("select count(*) as n from users").get() as { n: number }).n, 1);
  assert.deepEqual(migrations([USERS, BIO]).migrate(s), []);
  // The history table is reylo migrate's on SQLite: the file's name and SQL, and when it ran.
  const rows = s.db.prepare("select name, sql, applied_at is not null as applied, fingerprint, catalog from __reylo_migrations order by rowid").all();
  assert.deepEqual(rows.map((r) => ({ ...r })), [
    { name: USERS.name, sql: USERS.sql, applied: 1, fingerprint: null, catalog: null },
    { name: BIO.name, sql: BIO.sql, applied: 1, fingerprint: null, catalog: null },
  ]);
});

test("a migration that fails rolls back with its row, names itself, and leaves the ones before it run", () => {
  const s = storage();
  const broken = { name: "20261107120000_broken.sql", sql: "alter table users add column age int; alter table nowhere add column x int;" };
  assert.throws(() => runMigrations(s, [USERS, broken]), /^Error: reylo: the migration 20261107120000_broken\.sql failed on this Durable Object, and rolled back: no such table: nowhere/);
  assert.deepEqual(columns(s), ["id", "name"]);
  assert.deepEqual((s.db.prepare("select name from __reylo_migrations").all() as { name: string }[]).map((r) => r.name), [USERS.name]);
});

test("a history the bundle doesn't continue is refused before anything runs, naming the migration", () => {
  const AGE = { name: "20261108120000_age.sql", sql: "alter table users add column age int;" };
  const history = (s: { db: DatabaseSync }) => (s.db.prepare("select name from __reylo_migrations order by rowid").all() as { name: string }[]).map((r) => r.name);
  // A migration the object ran that the bundle lacks, as after a deploy rolled back past it.
  const s = storage();
  runMigrations(s, [USERS, BIO]);
  assert.throws(() => runMigrations(s, [USERS, AGE]), /^Error: reylo: this Durable Object ran the migration 20261106120000_bio\.sql, which this build doesn't have/);
  assert.deepEqual(history(s), [USERS.name, BIO.name]);
  assert.deepEqual(columns(s), ["id", "name", "bio"]);
  // One edited since the object ran it; trailing whitespace alone is no edit.
  assert.throws(() => runMigrations(s, [USERS, { ...BIO, sql: "alter table users add column bio text not null default '';" }, AGE]), /^Error: reylo: the migration 20261106120000_bio\.sql isn't the text this Durable Object ran/);
  assert.deepEqual(runMigrations(s, [USERS, { ...BIO, sql: BIO.sql + "\n\n" }]), []);
  // One the bundle's order puts before a migration the object already ran, as a branch merged late.
  const t = storage();
  runMigrations(t, [USERS, AGE]);
  assert.throws(() => runMigrations(t, [USERS, BIO, AGE]), /^Error: reylo: this Durable Object ran 20261108120000_age\.sql where this build's order has 20261106120000_bio\.sql first/);
  assert.deepEqual(columns(t), ["id", "name", "age"]);
});
