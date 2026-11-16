// The root module's `analyze`: each statement's descriptor in Reylo Type Defs, the same as
// `reylo analyze` prints for it, from the database the project names or the one the caller does.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { build } from "esbuild";
import { analyze, type TypeDefs } from "../src/index.ts";
import { nativeBinary } from "../src/native.ts";
import { mysqlDatabase, mysqlLive, pgLive, postgresDatabase, ROOT } from "./helpers.ts";

type StatementDescriptor = TypeDefs.StatementDescriptor;

/** What `reylo analyze --url <url> --sql <sql>` prints for one statement, run from a directory of its own. */
function cli(url: string, sql: string): StatementDescriptor {
  const cwd = mkdtempSync(join(tmpdir(), "reylo-analyze-cli-"));
  writeFileSync(join(cwd, "package.json"), "{}");
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => name !== "DATABASE_URL" && name !== "REYLO_CONFIG"));
  const run = spawnSync(nativeBinary(), ["analyze", "--url", url, "--sql", sql, "--format", "json"], { cwd, env, encoding: "utf8" });
  assert.ok(run.status === 0 || run.status === 1, run.stderr);
  const { file: _, ...descriptor } = JSON.parse(run.stdout.trim().split("\n")[0]!);
  return descriptor;
}

/** Each statement's descriptor from the API, checked against the command line's for it. */
async function same(url: string, statements: string[]): Promise<StatementDescriptor[]> {
  const found = await Promise.all(statements.map((sql) => analyze({ url, sql })));
  for (const [i, sql] of statements.entries()) assert.deepEqual(found[i], cli(url, sql), sql);
  return found;
}

function sqliteFile(setup: string): string {
  const file = join(mkdtempSync(join(tmpdir(), "reylo-analyze-api-")), "app.db");
  const db = new DatabaseSync(file);
  db.exec(setup);
  db.close();
  return file;
}

test("analyze returns a SQLite statement's descriptor in Reylo Type Defs, as reylo analyze prints it", async () => {
  const url = `file:${sqliteFile("create table users (id integer primary key, name text not null, email text)")}`;
  const [user, count, broken, untyped] = await same(url, ["-- One user, by id\nselect id, name, email from users where id = :id", "select count(*) as n from users", "select 'é🙂' as x, nope from users", "select 'é🙂' as x, :v as v, :w as w"]);
  const { result, ...rest } = user!;
  assert.deepEqual(rest, {
    $schema: "https://reylo.dev/typedefs/0.1.0.json",
    dialect: { name: "sqlite", version: user!.dialect.version },
    sql: "-- One user, by id\nselect id, name, email from users where id = ?",
    kind: "select",
    comment: "One user, by id",
    params: [{ name: "id", type: { kind: "integer", bits: 64 } }],
    capabilities: { tables: { users: "read" } },
    diagnostics: [],
  });
  assert.equal(result.kind, "table");
  assert.equal(result.cardinality, "at_most_one");
  assert.deepEqual(result.columns.map((c) => [c.name, c.nullable ?? false]), [["id", false], ["name", false], ["email", true]]);
  assert.match(user!.dialect.version, /^3\.\d+\.\d+$/);
  assert.equal(count!.result.cardinality, "one");
  // A diagnostic's place counts UTF-16 units of the statement as written, past characters of two
  // to four bytes.
  assert.deepEqual(broken!.diagnostics.map((d) => [d.message, "select 'é🙂' as x, nope from users".slice(d.start, d.end)]), [["no such column: nope", "nope"]]);
  assert.deepEqual(broken!.result.columns, []);
  // A parameter nothing types is an error at its first use, as under the project file's default,
  // `allow_untyped_params: false`.
  assert.deepEqual(untyped!.diagnostics.map((d) => [d.message, "select 'é🙂' as x, :v as v, :w as w".slice(d.start, d.end)]), ["v", "w"].map((n) => [`Nothing in the statement gives the parameter :${n} a type; cast it, as in :${n}::text, or set allow_untyped_params: true in .reylo.yaml.`, `:${n}`]));
  // Plain data: JSON carries all of it.
  assert.deepEqual(JSON.parse(JSON.stringify(user)), user);
});

test("analyze rejects a URL of no database it reads, and a database it cannot reach", async () => {
  await assert.rejects(analyze({ url: "redis://localhost", sql: "select 1" }), /Cannot tell the database from the URL/);
  await assert.rejects(analyze({ url: "postgres://postgres@127.0.0.1:1/none", sql: "select 1" }), /^Error: Cannot reach the database/);
  await assert.rejects(analyze({ url: "file:app.db" } as never), TypeError);
});

test("analyze reads Postgres, its server's version, comments and the server's own error codes", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_analyze_api", "create table users (id int8 primary key, email varchar(255) not null, bio text); comment on column users.bio is 'Shown on the profile page'");
  try {
    const [user, missing] = await same(db.url, ["select id, email, bio from users where id = :id", "select emial from users"]);
    const [{ server_version }] = await db.sql`show server_version`;
    assert.deepEqual(user!.dialect, { name: "postgres", version: server_version.split(" ")[0] });
    assert.equal(user!.sql, "select id, email, bio from users where id = $1");
    // Rows that can be many say which columns identify them.
    const [listed] = await same(db.url, ["select id, email from users order by id"]);
    assert.deepEqual([listed!.result.cardinality, listed!.result.unique], ["many", [["id"]]]);
    assert.deepEqual(user!.result.columns.map((c) => [c.name, c.type.kind, c.nullable ?? false, c.comment]), [["id", "integer", false, undefined], ["email", "text", false, undefined], ["bio", "text", true, "Shown on the profile page"]]);
    assert.deepEqual(missing!.diagnostics.map((d) => [d.code, "select emial from users".slice(d.start, d.end)]), [["42703", "emial"]]);
  } finally {
    await db.drop();
  }
});

test("analyze says what a SQLite statement may do: the tables it reaches, through views, foreign keys and triggers", async () => {
  const url = `file:${sqliteFile(`
    create table users (id integer primary key autoincrement, name text not null);
    create table posts (id integer primary key, author_id integer not null references users on delete cascade, title text not null);
    create table likes (post_id integer not null references posts, at text);
    create table audit (what text);
    create view titles as select p.title, u.name from posts p join users u on u.id = p.author_id;
    create trigger audit_likes after insert on likes begin insert into audit (what) values ('like'); end;
  `)}`;
  const [read, view, inserted, deleted, triggered, begin, pragma, ddl] = await same(url, [
    "select id, name from users where id = :id",
    "select title, name from titles",
    "insert into users (name) values (:name) returning id",
    "delete from users where id = :id",
    "insert into likes (post_id) values (:post)",
    "begin",
    "pragma foreign_keys = on",
    "create table more (id integer primary key)",
  ]);
  assert.deepEqual(read!.capabilities, { tables: { users: "read" } });
  // A view reads the tables it is made of.
  assert.deepEqual(view!.capabilities, { tables: { posts: "read", titles: "read", users: "read" } });
  // An AUTOINCREMENT table's insert counts in sqlite_sequence.
  assert.deepEqual(inserted!.capabilities, { tables: { sqlite_sequence: "write", users: "write" } });
  // A foreign key's ON DELETE CASCADE deletes from the table that refers, whose own key's check reads the next.
  assert.deepEqual(deleted!.capabilities, { tables: { likes: "read", posts: "write", users: "write" } });
  // A trigger's body is not read: any table.
  assert.deepEqual(triggered!.capabilities, { tables: "unknown", ddl: true, session: true, external: true });
  assert.deepEqual(begin!.capabilities, { tables: {}, transaction: true });
  assert.deepEqual(pragma!.capabilities, { tables: {}, ddl: true, session: true });
  assert.deepEqual(ddl!.capabilities, { tables: "unknown", ddl: true });
});

test("analyze says what a Postgres statement may do: the tables it reaches, and what it does beyond their rows", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_analyze_api_capabilities", `
    create table users (id int8 primary key generated always as identity, email text not null);
    create table posts (id int8 primary key generated always as identity, author_id int8 not null references users on delete cascade, title text not null);
    create table jobs (id int8 primary key, done bool not null default false);
    create table audit (what text);
    create view titles as select p.title, u.email from posts p join users u on u.id = p.author_id;
    create function log_job() returns trigger language plpgsql as $$ begin insert into audit (what) values ('job'); return new; end $$;
    create trigger jobs_audit after update on jobs for each row execute function log_job();
    create sequence tickets;
  `);
  try {
    const [read, view, inserted, deleted, locked, triggered, ticket, set, begin, notify, ddl] = await same(db.url, [
      "select id, email from users where id = :id",
      "select title, email from titles",
      "insert into users (email) values (:email) returning id",
      "delete from users where id = :id",
      "select id from jobs where not done for update",
      "update jobs set done = true where id = :id",
      "select nextval('tickets') as n",
      "select set_config('app.user', :user::text, false)",
      "begin",
      "select pg_notify('jobs', :what::text)",
      "create table more (id int primary key)",
    ]);
    assert.deepEqual(read!.capabilities, { tables: { "public.users": "read" } });
    // A view reads the tables it is made of.
    assert.deepEqual(view!.capabilities, { tables: { "public.posts": "read", "public.titles": "read", "public.users": "read" } });
    // An identity column's sequence is written by the insert.
    assert.deepEqual(inserted!.capabilities, { tables: { "public.users": "write", "public.users_id_seq": "write" } });
    // ON DELETE CASCADE deletes from the table that refers, and the key's check locks the rows it reads.
    assert.deepEqual(deleted!.capabilities, { tables: { "public.posts": "write", "public.users": "write" } });
    assert.deepEqual(locked!.capabilities, { tables: { "public.jobs": "readlock" } });
    // A trigger's body is not read: any table, and anything a function may do but end the transaction.
    assert.deepEqual(triggered!.capabilities, { tables: "unknown", ddl: true, session: true, external: true });
    assert.deepEqual(ticket!.capabilities, { tables: { "public.tickets": "write" } });
    assert.deepEqual(set!.capabilities, { tables: {}, session: true });
    assert.deepEqual(begin!.capabilities, { tables: {}, transaction: true });
    assert.deepEqual(notify!.capabilities, { tables: {}, external: true });
    // DDL sets off any event trigger, whose code is not read.
    assert.deepEqual(ddl!.capabilities, { tables: "unknown", ddl: true, session: true, external: true });
  } finally {
    await db.drop();
  }
});

test("analyze reads the catalog as it stands at each call, though calls share a connection", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_analyze_api_fresh", "create table notes (id int primary key, body text)");
  try {
    const before = await analyze({ url: db.url, sql: "select body from notes" });
    assert.equal(before.result.columns[0]!.nullable, true);
    await db.sql`alter table notes alter column body set not null`;
    const after = await analyze({ url: db.url, sql: "select body from notes" });
    assert.equal(after.result.columns[0]!.nullable, undefined);
  } finally {
    await db.drop();
  }
});

test("analyze reads MySQL, its server's version and the server's own error codes", { skip: mysqlLive }, async () => {
  const db = await mysqlDatabase("reylo_analyze_api", "create table users (id bigint primary key, email varchar(255) not null, bio text comment 'Shown on the profile page')");
  try {
    const [user, missing] = await same(db.url, ["select id, email, bio from users where id = :id", "select emial from users"]);
    const [{ v }] = (await db.rows("select version() as v")) as [{ v: string }];
    assert.deepEqual(user!.dialect, { name: "mysql", version: v });
    assert.equal(user!.sql, "select id, email, bio from users where id = ?");
    assert.equal(user!.result.cardinality, "at_most_one");
    assert.deepEqual(user!.result.columns.map((c) => [c.name, c.nullable ?? false, c.comment]), [["id", false, undefined], ["email", false, undefined], ["bio", true, "Shown on the profile page"]]);
    assert.deepEqual(missing!.diagnostics.map((d) => d.code), ["1054"]);
  } finally {
    await db.drop();
  }
});

test("analyze says what a MySQL statement may do: the tables it reaches, and what it does beyond their rows", { skip: mysqlLive }, async () => {
  const db = await mysqlDatabase("reylo_analyze_api_capabilities", `
    create table users (id bigint primary key auto_increment, email varchar(255) not null);
    create table posts (id bigint primary key auto_increment, author_id bigint not null, title text not null, foreign key (author_id) references users (id) on delete cascade);
    create table jobs (id bigint primary key, done bool not null default false);
    create table audit (what text);
    create view titles as select p.title, u.email from posts p join users u on u.id = p.author_id;
    create trigger jobs_audit after update on jobs for each row insert into audit (what) values ('job');
  `);
  try {
    const [read, view, deleted, locked, triggered, set, begin, ddl] = await same(db.url, [
      "select id, email from users where id = :id",
      "select title, email from titles",
      "delete from users where id = :id",
      "select id from jobs where not done for update",
      "update jobs set done = true where id = :id",
      "set @user = 1",
      "begin",
      "create table more (id int primary key)",
    ]);
    const name = db.url.split("/").pop()!.split("?")[0]!;
    const t = (table: string) => `${name}.${table}`;
    assert.deepEqual(read!.capabilities, { tables: { [t("users")]: "read" } });
    // A view reads the tables it is made of.
    assert.deepEqual(view!.capabilities, { tables: { [t("posts")]: "read", [t("titles")]: "read", [t("users")]: "read" } });
    // ON DELETE CASCADE deletes from the table that refers.
    assert.deepEqual(deleted!.capabilities, { tables: { [t("posts")]: "write", [t("users")]: "write" } });
    assert.deepEqual(locked!.capabilities, { tables: { [t("jobs")]: "readlock" } });
    // A trigger's body is not read: any table.
    assert.deepEqual(triggered!.capabilities, { tables: "unknown", ddl: true, session: true, external: true });
    assert.deepEqual(set!.capabilities, { tables: {}, session: true });
    assert.deepEqual(begin!.capabilities, { tables: {}, transaction: true });
    // DDL commits the open transaction.
    assert.deepEqual(ddl!.capabilities, { tables: "unknown", ddl: true, transaction: true });
  } finally {
    await db.drop();
  }
});

/** A package with `files` written into it, and the SQLite databases `databases` names, each holding one table of its name. */
function project(files: Record<string, string>, databases: string[] = []): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-analyze-api-project-"));
  writeFileSync(join(root, "package.json"), "{}");
  for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
  for (const name of databases) {
    const db = new DatabaseSync(join(root, `${name}.db`));
    db.exec(`create table ${name} (id integer primary key)`);
    db.close();
  }
  return root;
}

/** `work`, run from `dir` with no connection of the developer's own in the environment, as a script in that project runs. */
async function inProject<T>(dir: string, work: () => Promise<T>): Promise<T> {
  const cwd = process.cwd();
  const saved = { DATABASE_URL: process.env.DATABASE_URL, REYLO_CONFIG: process.env.REYLO_CONFIG };
  delete process.env.DATABASE_URL;
  delete process.env.REYLO_CONFIG;
  process.chdir(dir);
  try {
    return await work();
  } finally {
    process.chdir(cwd);
    for (const [name, value] of Object.entries(saved)) if (value !== undefined) process.env[name] = value;
  }
}

/** What `reylo analyze --sql <sql>` prints from `cwd`, with no connection of the developer's own in its environment. */
function cliIn(cwd: string, sql: string, ...args: string[]): StatementDescriptor {
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => name !== "DATABASE_URL" && name !== "REYLO_CONFIG"));
  const run = spawnSync(nativeBinary(), ["analyze", "--sql", sql, "--format", "json", ...args], { cwd, env, encoding: "utf8" });
  assert.ok(run.status === 0 || run.status === 1, run.stderr);
  return JSON.parse(run.stdout.trim().split("\n")[0]!);
}

test("analyze reads the database the project names when no url is given, as the type check and reylo analyze do", async () => {
  // The project file's direct_url, from the project's root, over DATABASE_URL in its .env.
  const named = project({ ".reylo.yaml": "direct_url: file:./named.db\n", ".env": "DATABASE_URL=file:./other.db\n" }, ["named", "other"]);
  const fromFile = await inProject(named, () => analyze({ sql: "select id from named" }));
  assert.deepEqual(fromFile.diagnostics, []);
  assert.deepEqual(fromFile, cliIn(named, "select id from named"));
  // DATABASE_URL from a .env file, with no project file.
  const dotenv = project({ ".env": "DATABASE_URL=file:./other.db\n" }, ["other"]);
  const fromEnv = await inProject(dotenv, () => analyze({ sql: "select id from other" }));
  assert.deepEqual(fromEnv.diagnostics, []);
  assert.deepEqual(fromEnv, cliIn(dotenv, "select id from other"));
  // A url given wins, and its relative path is read from the working directory.
  const given = await inProject(named, () => analyze({ sql: "select id from other", url: "file:./other.db" }));
  assert.deepEqual(given.diagnostics, []);
  assert.deepEqual(given, cliIn(named, "select id from other", "--url", "file:./other.db"));
});

test("analyze with a url leaves the project's connection unread, so a command in it never runs", async () => {
  const root = project({ ".reylo.yaml": "direct_url: $(exit 7)\n" }, ["other"]);
  await inProject(root, async () => {
    await assert.rejects(analyze({ sql: "select id from other" }), /direct_url/);
    assert.deepEqual((await analyze({ sql: "select id from other", url: "file:./other.db" })).diagnostics, []);
  });
});

test("analyze says where to set a database when none is named, and what is wrong with a project file it cannot read", async () => {
  await inProject(project({}), async () => {
    await assert.rejects(analyze({ sql: "select 1" }), /^Error: No database URL: set DATABASE_URL in the environment, \.env\.local or \.env, or "direct_url" in \.reylo\.yaml\.$/);
  });
  await inProject(project({ ".reylo.yaml": "nope: 1\n" }, ["other"]), async () => {
    await assert.rejects(analyze({ sql: "select 1", url: "file:./other.db" }), /\.reylo\.yaml:1:1/);
  });
});

test("analyze holds an untyped parameter to the project file's allow_untyped_params, with a url given too", async () => {
  const strict = project({ ".env": "DATABASE_URL=file:./other.db\n" }, ["other"]);
  const refused = await inProject(strict, () => analyze({ sql: "select :x as x" }));
  assert.deepEqual(refused.diagnostics.map((d) => d.message), ["Nothing in the statement gives the parameter :x a type; cast it, as in :x::text, or set allow_untyped_params: true in .reylo.yaml."]);
  assert.deepEqual(refused, cliIn(strict, "select :x as x"));
  const lax = project({ ".reylo.yaml": "allow_untyped_params: true\n", ".env": "DATABASE_URL=file:./other.db\n" }, ["other"]);
  for (const input of [{ sql: "select :x as x" }, { sql: "select :x as x", url: "file:./other.db" }]) {
    const found = await inProject(lax, () => analyze(input));
    assert.deepEqual(found.diagnostics, []);
    assert.deepEqual(found.params, [{ name: "x", type: { kind: "unknown" } }]);
  }
  assert.deepEqual(await inProject(lax, () => analyze({ sql: "select :x as x" })), cliIn(lax, "select :x as x"));
});

test("an app's bundle of the root module holds none of the build-time half, and calling analyze where there is no Node says so", async () => {
  const { outputFiles, warnings, metafile } = await build({ entryPoints: [join(ROOT, "src", "index.ts")], bundle: true, platform: "browser", format: "esm", write: false, metafile: true, logLevel: "silent" });
  assert.deepEqual(warnings, []);
  const inputs = Object.keys(metafile.inputs);
  for (const part of ["src/tooling.ts", "src/analyze-api.ts", "src/engine.ts", "src/analyze.ts", "src/project.ts"]) assert.ok(!inputs.includes(part), `${part} is in the bundle`);
  assert.ok(inputs.includes("src/build-time.ts"));
  assert.doesNotMatch(outputFiles[0]!.text, /child_process|import\.meta/);
  // A browser, or a Worker without nodejs_compat: no `process` to reach Node through.
  const saved = Object.getOwnPropertyDescriptor(globalThis, "process")!;
  Object.defineProperty(globalThis, "process", { value: undefined, configurable: true });
  try {
    await assert.rejects(analyze({ sql: "select 1" }), /needs Node 24 or later, or Bun/);
  } finally {
    Object.defineProperty(globalThis, "process", saved);
  }
});
