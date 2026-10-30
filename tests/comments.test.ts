import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { fieldComment } from "../src/module-types.ts";
import { DRIVERS, MYSQL, pgLive, mysqlLive, ROOT, declarations, postgresDatabase, languageServer } from "./helpers.ts";

function project(url: string, driver: string, dialect: string): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-comments-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  mkdirSync(join(root, "node_modules", "@reylosql"), { recursive: true });
  symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  symlinkSync(join(ROOT, "packages", dialect), join(root, "node_modules", "@reylosql", dialect));
  return root;
}

/** The `Params` and `Row` a declaration exports. */
function types(text: string): { params: string; row: string } {
  const params = /^export type Params = ([\s\S]*?);\nexport type Row/m.exec(text)![1]!;
  const row = /^export type Row = ([\s\S]*?);\n(?=\/\*\*|export )/m.exec(text)![1]!;
  return { params, row };
}

const POSTGRES_SCHEMA = `create table users (id int primary key, age int, email text not null, note text);
  comment on column users.id is 'The user''s id';
  comment on column users.age is 'Age (in years)';
  comment on column users.email is E'Where we write to them.\\nNever shown to other users.';
  comment on column users.note is 'Free text; a */ ends nothing';
  create view adults as select id, age as years from users where age >= 18;
  comment on column adults.id is 'An adult''s id'`;

test("a doc comment is one line, or a block of lines, with a star and a slash kept apart", () => {
  assert.equal(fieldComment(undefined), "");
  assert.equal(fieldComment(" \n "), "");
  assert.equal(fieldComment("Age (in years)"), "/** Age (in years) */");
  assert.equal(fieldComment("a */ b"), "/** a *\\/ b */");
  assert.equal(fieldComment("Line 1\r\nLine 2\n\nLine 4  \n", "  "), "/**\n   * Line 1\n   * Line 2\n   *\n   * Line 4\n   */");
});

test("Postgres: a field that reads a column as it is carries the column's comment, which a changed comment re-types", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_comments", POSTGRES_SCHEMA);
  try {
    const root = project(db.url, "postgres", "postgres");
    const files = {
      "user.sql": "select id, age, age as years, email, note, lower(email) as lower_email, count(*) over () as total from users where id = :id\n",
      "adults.sql": "select id, years from adults\n",
      "update.sql": "update users set age = :age, note = :note where id = :id returning age\n",
    };
    const out = await declarations(root, files);
    // An alias keeps the column's comment; an expression over a column has none. A type with a
    // doc comment has a field per line, since TypeScript reads one only where a line starts with it.
    assert.deepEqual(types(out["user.sql"]!), {
      params: "{ id: number }",
      row: `{
  /** The user's id */
  id: number;
  /** Age (in years) */
  age: number | null;
  /** Age (in years) */
  years: number | null;
  /**
   * Where we write to them.
   * Never shown to other users.
   */
  email: string;
  /** Free text; a *\\/ ends nothing */
  note: string | null;
  lower_email: string;
  total: bigint;
}`,
    });
    // Through a view: the view's column's own comment, else the one of the table column it reads.
    assert.equal(types(out["adults.sql"]!).row, "{\n  /** An adult's id */\n  id: number;\n  /** Age (in years) */\n  years: number;\n}");
    // A parameter written to a column carries the column's comment.
    assert.deepEqual(types(out["update.sql"]!), {
      params: "{\n  /** Age (in years) */\n  age?: number | null;\n  /** Free text; a *\\/ ends nothing */\n  note?: string | null;\n  id: number;\n}",
      row: "{\n  /** Age (in years) */\n  age: number | null;\n}",
    });

    // The schema's fingerprint takes in the comments, so the entry `.reylo/` keeps is analyzed again.
    await db.sql.unsafe("comment on column users.age is 'Age, in whole years'");
    const again = await declarations(root, { "user.sql": files["user.sql"] });
    assert.match(types(again["user.sql"]!).row, /\/\*\* Age, in whole years \*\/\n  age: number \| null;\n  \/\*\* Age, in whole years \*\/\n  years/);
  } finally {
    await db.drop();
  }
});

test("MySQL: a field that reads a column as it is carries the column's comment, which a changed comment re-types", { skip: mysqlLive }, async () => {
  const mysql = createRequire(join(DRIVERS, "mysql2", "package.json"))("mysql2/promise");
  const admin = await mysql.createConnection(MYSQL!);
  const name = `reylo_comments_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(MYSQL!);
  url.pathname = `/${name}`;
  const db = await mysql.createConnection(url.href);
  try {
    await db.query(`create table users (id int primary key comment 'The user''s id', age int comment 'Age (in years)', email varchar(100) not null comment 'Where we write to them.\\nNever shown to other users.', note text)`);
    await db.query(`create table other (id int primary key comment 'Another id', age int comment 'Another age')`);
    const root = project(url.href, "mysql2", "mysql");
    const files = {
      "user.sql": "select id, age as years, email, note, upper(email) as shout from users where id = :id\n",
      "derived.sql": "select age from (select id as age from other) o\n",
      "update.sql": "update users set age = :age where id = :id\n",
    };
    const out = await declarations(root, files);
    // An alias keeps the column's comment, an expression has none, and so has a column without one.
    // A parameter compared with a column carries its comment too.
    assert.deepEqual(types(out["user.sql"]!), {
      params: "{\n  /** The user's id */\n  id: number;\n}",
      row: `{
  /** The user's id */
  id: number;
  /** Age (in years) */
  years: number | null;
  /**
   * Where we write to them.
   * Never shown to other users.
   */
  email: string;
  note: string | null;
  shout: string | null;
}`,
    });
    // Through a derived table MySQL names the base table beside the derived column's own name,
    // here another column of it: no comment.
    assert.equal(types(out["derived.sql"]!).row, "{ age: number }");
    assert.equal(types(out["update.sql"]!).params, "{\n  /** Age (in years) */\n  age?: number | null;\n  /** The user's id */\n  id: number;\n}");

    await db.query("alter table users modify age int comment 'Age, in whole years'");
    const again = await declarations(root, { "user.sql": files["user.sql"] });
    assert.match(types(again["user.sql"]!).row, /\/\*\* Age, in whole years \*\/\n  years: number \| null;/);
  } finally {
    await db.end();
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});

test("the hover over a field of a query's row shows its column's comment", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_hover", POSTGRES_SCHEMA);
  const lsp = { close: async () => {} } as ReturnType<typeof languageServer>;
  try {
    const root = project(db.url, "postgres", "postgres");
    const compilerOptions = { target: "esnext", module: "nodenext", lib: ["esnext"], types: [], strict: true, noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true };
    writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ compilerOptions, contentMappers: [{ package: "reylo", extensions: [".sql"] }], include: ["main.ts"] }));
    writeFileSync(join(root, "user.sql"), "select id, age, email from users where id = :id\n");
    const main = join(root, "main.ts");
    writeFileSync(main, `import getUser from "./user.sql";\n\nexport async function age(db: Parameters<typeof getUser.run>[0]) {\n  const user = await getUser.run(db, { id: 1 });\n  return user?.age;\n}\n`);
    const text = readFileSync(main, "utf8");
    const before = text.slice(0, text.indexOf("user?.age") + "user?.".length).split("\n");
    const position = { line: before.length - 1, character: before.at(-1)!.length + 1 };
    Object.assign(lsp, languageServer(root));
    await lsp.request("initialize", { processId: process.pid, rootUri: pathToFileURL(root).href, capabilities: { textDocument: { hover: { contentFormat: ["markdown", "plaintext"] } } }, initializationOptions: { runExternalCode: true } });
    lsp.notify("initialized", {});
    const uri = pathToFileURL(main).href;
    lsp.notify("textDocument/didOpen", { textDocument: { uri, languageId: "typescript", version: 1, text } });
    // Until the mapper has analyzed the statement, the field is no property of anything.
    let hover = "";
    for (let i = 0; i < 100 && !hover.includes("Age (in years)"); i++) {
      if (i) await new Promise((resolve) => setTimeout(resolve, 200));
      hover = (await lsp.request("textDocument/hover", { textDocument: { uri }, position }))?.contents?.value ?? "";
    }
    assert.equal(hover, "```typescript\n(property) age: number | null | undefined\n```\nAge (in years)");
  } finally {
    await lsp.close();
    await db.drop();
  }
});
