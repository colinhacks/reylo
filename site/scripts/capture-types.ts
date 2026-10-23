// `nub run capture` in site/: the module Reylo's content mapper returns for every `.sql` block a
// `ts twoslash` block of the docs imports, written to lib/sql-modules.json, which
// lib/remark-mapper.ts hides above each block's `// ---cut---` at build time.
//
//   nub run capture                          every page: content/docs, with content/sections
//   nub run capture -- content/drafts/x.mdx  those pages too
//   nub run capture -- --schema mysql=f.sql  a dialect's schema, for a page that shows none
//
// For each page and dialect it builds the page's schema (the `schema.sql` blocks, one per
// dialect, in the accordion near the top) in a scratch database: Postgres and MySQL
// `reylo_capture` (REYLO_CAPTURE_POSTGRES and REYLO_CAPTURE_MYSQL name others; the database is
// dropped and made again, so its name must start with reylo_capture), SQLite a temporary file. It
// compiles the syntax the engine does not build yet (typed JSON parameters, `::` on MySQL and
// SQLite), has the loader's own Session analyze each statement through the dialect's driver, and
// renders the module exactly as src/mapper.ts does, with each typed JSON parameter's type computed
// from its schema (scripts/schema-type.ts) where the loader still says `unknown`. An error in a
// statement a block imports fails the run; one in a statement nothing imports is a warning.
//
// Each error example (a `sql` block marked `errors`, lib/sql-errors.ts) goes to the content mapper
// itself, `src/mapper.ts` started as TypeScript starts it (scripts/mapper-diagnostics.ts), on the
// same schema, and the errors it gives the file, where the editor and `tsc` place them, are written
// to lib/sql-errors.json; an example the mapper finds no error in fails the run.
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import postgres from 'postgres';
import mysql from 'mysql2/promise';
import { Session } from '../../src/analyze.ts';
import { ident, renderParamType, renderType } from '../../src/render.ts';
import { docComment, leadingComment, RAW_NOTE, sqlNote, statementAfter } from '../../src/doc-comment.ts';
import { withSqlcommenter } from '../../src/sqlcommenter.ts';
import type { Analysis } from '../../src/engine.ts';
import { collectFiles, plan, isStatement, DIALECT_NAME, SCHEMA_FILE, type Block, type Dialect, type Modules } from '../lib/remark-mapper.ts';
import { sitePages } from '../lib/page-files.ts';
import { isErrorExample, type Errors } from '../lib/sql-errors.ts';
import { schemaType } from './schema-type.ts';
import { openMapper, positionOf } from './mapper-diagnostics.ts';

const site = path.resolve(import.meta.dirname, '..');
const URLS: Record<Exclude<Dialect, 'sqlite'>, string> = {
  postgres: process.env.REYLO_CAPTURE_POSTGRES ?? 'postgres://postgres@localhost:54329/reylo_capture',
  mysql: process.env.REYLO_CAPTURE_MYSQL ?? 'mysql://root@127.0.0.1:54336/reylo_capture',
};
/** The scratch database an `<Inferred>` card's own schema is built in, on the same server: made again for each card. */
const cardUrl = (url: string) => {
  const u = new URL(url);
  u.pathname = `/${decodeURIComponent(u.pathname.slice(1))}_card`;
  return u.href;
};
// The driver each scratch project names, whose defaults the types follow, as for a project with
// that driver: postgres.js, mysql2, and node:sqlite (no package).
const DRIVER: Record<Dialect, string | undefined> = { postgres: 'postgres', mysql: 'mysql2', sqlite: undefined };
const ORDER: Dialect[] = ['postgres', 'mysql', 'sqlite'];

// ---- Arguments ----

const files: string[] = [];
const schemaFiles: Partial<Record<Dialect, string>> = {};
const args = process.argv.slice(2);
for (let i = 0; i < args.length; i++) {
  const arg = args[i]!;
  if (arg === '--') continue;
  const schema = /^--schema(?:=(.*))?$/.exec(arg);
  if (schema) {
    const value = schema[1] ?? args[++i];
    const m = /^(postgres|mysql|sqlite)=(.+)$/.exec(value ?? '');
    if (!m) fail(`--schema takes <dialect>=<file>, as in --schema mysql=schema.sql; got ${value ?? 'nothing'}`);
    schemaFiles[m[1] as Dialect] = readFileSync(path.resolve(m[2]!), 'utf8');
  } else if (arg.startsWith('-')) fail(`unknown option ${arg}`);
  else files.push(path.resolve(arg));
}
// The site's pages (lib/page-files.ts): the main page is its files together, the opening with the
// schema and then a file per section, whose imports resolve across all of them; every other file
// of content/docs, and each file named on the command line, is a page by itself.
const ownPages = sitePages(site);
const known = new Set(ownPages.flat());
const pages: string[][] = [...ownPages, ...files.filter((file) => !known.has(file)).map((file) => [file])];

function fail(message: string): never {
  console.error(`capture: ${message}`);
  process.exit(1);
}

// ---- The syntax the engine does not build yet ----

const NAME = String.raw`:[A-Za-z_]\w*`;
const SCHEMA_STRING = String.raw`'((?:[^']|'')*)'`;

/** Each typed JSON parameter of the statement as written: its name and its schema. */
function jsonParams(sql: string): { name: string; schema: unknown }[] {
  return [...sql.matchAll(new RegExp(String.raw`\(?(${NAME})\)?::jsonb?\(${SCHEMA_STRING}\)`, 'g'))].map((m) => {
    const text = m[2]!.replace(/''/g, "'");
    try {
      return { name: m[1]!.slice(1), schema: JSON.parse(text) };
    } catch (e) {
      throw new Error(`the schema of ${m[1]} is not JSON: ${(e as Error).message}`);
    }
  });
}

/**
 * The statement the engine reads: Postgres `:x::jsonb('…')` → `:x::jsonb`; MySQL `(:x)::json('…')`
 * → `cast(:x as json)`; SQLite `:x::json('…')` → `:x` (each with or without the parentheses). A
 * plain `::type` stays as written: since 2026-10-20 the engine writes it as MySQL's or SQLite's own
 * cast, `CAST(? AS CHAR)` for `::text` on MySQL, which a rewrite to `cast(:x as text)` here broke.
 */
function compile(sql: string, dialect: Dialect): string {
  if (dialect === 'postgres') return sql.replace(new RegExp(String.raw`::(jsonb?)\(${SCHEMA_STRING}\)`, 'g'), '::$1');
  // The parentheses come in a pair or not at all: a call's own, as in `json_table(:x::json('…'), …)`, stays.
  const json = new RegExp(String.raw`(?:\((${NAME})\)|(${NAME}))::json\(${SCHEMA_STRING}\)`, 'g');
  return sql.replace(json, (_, wrapped: string | undefined, bare: string | undefined) => (dialect === 'mysql' ? `cast(${wrapped ?? bare} as json)` : (wrapped ?? bare)!));
}

// ---- The module, as src/mapper.ts renders it ----

/**
 * The module src/mapper.ts's `transform` returns for the statement, copied from there (keep the two
 * in step): `written` is the file's text, declared as `raw`, and `sql` what the engine reads; the
 * module's `sql` is the engine's compiled text, the statement as Reylo sends it to the database.
 * One change: a typed JSON parameter takes the type computed from its schema (`json`), where the
 * loader says `unknown`.
 */
async function render(session: Session, source: string, written: string, sql: string, json: Map<string, string>, strict = session.project.strictParams): Promise<{ text: string; analysis: Analysis } | { error: string }> {
  const project = session.project;
  const paramType = (q: Analysis['params'][number], rendered: string): string => {
    if (json.has(q.name)) return json.get(q.name)!;
    if (q.type.kind === 'unknown') return 'unknown';
    return q.nullable ? `${rendered} | null` : rendered;
  };
  const emit = (compiled: string, paramsType: string, rowType: string, cardinality: string, names: string[], plain = compiled) => {
    const leading = leadingComment(written, project.dialect);
    const prose = leading.prose;
    const shown = statementAfter(written, leading);
    const sent = statementAfter(plain, leadingComment(plain, project.dialect));
    const sqlDoc = (indent = '') => docComment(prose, sent, sqlNote(project.sqlcommenter), indent);
    const rawDoc = (indent = '') => docComment(prose, shown, RAW_NOTE, indent);
    return [
      `import type { Query } from "reylo";`,
      `export type Params = ${paramsType};`,
      `export type Row = ${rowType};`,
      `${sqlDoc()}export declare const sql: ${JSON.stringify(compiled)};`,
      `${rawDoc()}export declare const raw: ${JSON.stringify(written)};`,
      `export declare const params: readonly [${names.map((n) => JSON.stringify(n)).join(', ')}];`,
      `${docComment(prose, shown)}declare const query: Query<${paramsType}, ${rowType}, ${JSON.stringify(cardinality)}, ${JSON.stringify(project.dialect)}> & {`,
      `${sqlDoc('  ')}  readonly sql: typeof sql;`,
      `${rawDoc('  ')}  readonly raw: typeof raw;`,
      `  readonly source: string;`,
      `};`,
      `export default query;`,
      ``,
    ].join('\n');
  };
  let names: string[] = [];
  try {
    ({ params: names } = session.lex(sql));
  } catch {}
  const outcome = await session.analyze(sql);
  if (!outcome.ok) return { error: outcome.message };
  const { analysis } = outcome;
  const dialect = project.dialect;
  if (dialect === 'mysql') names = analysis.params.map((q) => q.name);
  // The mapper places these in the file as errors; a statement on the page must have none.
  if (analysis.diagnostics.length) return { error: analysis.diagnostics.map((d) => d.message).join('; ') };
  if (strict) {
    const untyped = analysis.params.filter((q) => q.type.kind === 'unknown' && !json.has(q.name));
    if (untyped.length) return { error: `strict_params: nothing types ${untyped.map((q) => `:${q.name}`).join(', ')}` };
  }
  const inferred = analysis.cardinality;
  const order = names;
  if (dialect === 'postgres' && analysis.kind === '') return { error: 'the statement is empty' };
  const columns = analysis.columns.map((c) => `${ident(c.name)}: ${renderType(c.type, false, dialect)}${c.nullable ? ' | null' : ''}`);
  const typed = analysis.params.map((q) => [q.name, paramType(q, renderParamType(q.type, dialect))] as const);
  // A parameter that takes null may be left out, and so may a typed JSON one whose schema has a `default`.
  const optional = new Set(analysis.params.filter((q) => q.nullable || (q.schema !== null && typeof q.schema === 'object' && 'default' in q.schema)).map((q) => q.name));
  const paramTypes = typed.map(([name, type]) => `${ident(name)}${optional.has(name) ? '?' : ''}: ${type}`);
  return { text: emit(project.sqlcommenter ? withSqlcommenter(analysis.rewritten.sql, source) : analysis.rewritten.sql, paramTypes.length ? `{ ${paramTypes.join('; ')} }` : 'Record<never, never>', columns.length ? `{ ${columns.join('; ')} }` : 'never', inferred, order, analysis.rewritten.sql), analysis };
}

// ---- The scratch databases ----

/** A Postgres or MySQL URL's database, which the run drops and makes again: only a scratch one. */
function scratch(url: string): { db: string; admin: URL } {
  const u = new URL(url);
  const db = decodeURIComponent(u.pathname.slice(1));
  if (!/^reylo_capture\w*$/.test(db)) fail(`${url} names the database "${db}"; the capture drops and makes its database again, so its name must start with reylo_capture`);
  return { db, admin: u };
}

async function buildPostgres(schema: string, url = URLS.postgres): Promise<string> {
  const { db, admin } = scratch(url);
  admin.pathname = '/postgres';
  const server = postgres(admin.href, { max: 1, onnotice: () => {} });
  try {
    await server.unsafe(`drop database if exists "${db}" with (force)`);
    await server.unsafe(`create database "${db}"`);
  } finally {
    await server.end();
  }
  const sql = postgres(url, { max: 1, onnotice: () => {} });
  try {
    await sql.unsafe(schema).simple();
  } finally {
    await sql.end();
  }
  return url;
}

async function buildMysql(schema: string, url = URLS.mysql): Promise<string> {
  const { db, admin } = scratch(url);
  admin.pathname = '/';
  const conn = await mysql.createConnection({ uri: admin.href, multipleStatements: true });
  try {
    await conn.query(`drop database if exists \`${db}\``);
    await conn.query(`create database \`${db}\``);
    await conn.query(`use \`${db}\``);
    await conn.query(schema);
  } finally {
    await conn.end();
  }
  return url;
}

function buildSqlite(dir: string, schema: string): string {
  const db = new DatabaseSync(path.join(dir, 'capture.db'));
  try {
    db.exec(schema);
  } finally {
    db.close();
  }
  return 'file:./capture.db';
}

/**
 * A project of its own per dialect, as an app using Reylo has: its `package.json` names the driver
 * (which picks the types' defaults), and the driver resolves from the site's node_modules.
 */
function project(root: string, dialect: Dialect, name: string = dialect): string {
  const dir = path.join(root, name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const driver = DRIVER[dialect];
  writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name: `reylo-capture-${dialect}`, private: true, type: 'module', dependencies: driver ? { [driver]: '*' } : {} }, null, 2));
  symlinkSync(path.join(site, 'node_modules'), path.join(dir, 'node_modules'), 'dir');
  return dir;
}

// ---- The run ----

const modules: Modules = {};
const failures: Errors = {};
const errors: string[] = [];
const warnings: string[] = [];
const root = mkdtempSync(path.join(tmpdir(), 'reylo-capture-'));
let captured = 0;
let analyzed = 0;

try {
  for (const page of pages) {
    const shown = path.relative(site, page[0]!).startsWith('..') ? page[0]! : path.relative(site, page[0]!);
    let blocks: Block[];
    let needed: Set<Block>;
    try {
      blocks = collectFiles(page, site);
      needed = new Set(plan(blocks).flatMap((p) => p.variants.flatMap((v) => v.needs.map((n) => n.block))));
      // An <Inferred> card shows its statement's types, so its module is kept whether or not a block imports it.
      for (const block of blocks) if (block.inferred) needed.add(block);
    } catch (e) {
      errors.push(`${shown}: ${(e as Error).message}`);
      continue;
    }
    const statements = blocks.filter(isStatement);
    if (!statements.length) continue;
    const schemas: Partial<Record<Dialect, string>> = { ...schemaFiles };
    for (const d of ORDER) {
      const own = blocks.filter((b) => b.name === SCHEMA_FILE && b.dialect === d);
      if (own.length) schemas[d] = own.map((b) => b.node.value).join('\n\n');
    }
    for (const dialect of ORDER) {
      const mine = statements.filter((b) => b.dialect === dialect);
      if (!mine.length) continue;
      const report = (block: Block, message: string) => (needed.has(block) ? errors : warnings).push(`${block.file ?? shown}:${block.line}: ${DIALECT_NAME[dialect]} ${block.name}: ${message}\n${indent(block.node.value)}`);
      const schema = schemas[dialect];
      if (schema === undefined) {
        for (const block of mine) report(block, `the page has no ${DIALECT_NAME[dialect]} ${SCHEMA_FILE} block to build its schema from; add one, or pass --schema ${dialect}=<file>`);
        continue;
      }
      // One statement's module, from a session on the schema it is analyzed against.
      const analyze = async (session: Session, block: Block) => {
        const written = block.node.value;
        let sql: string;
        const json = new Map<string, string>();
        try {
          sql = compile(written, dialect);
          for (const p of jsonParams(written)) json.set(p.name, schemaType(p.schema as never, dialect, `:${p.name}`));
        } catch (e) {
          report(block, (e as Error).message);
          return;
        }
        // A block that shows what `strict_params: false` gives says so with `strict="false"`, and is
        // analyzed as such a project would be; every other block must type each parameter.
        const strict = !/(?:^|\s)strict="false"(?:\s|$)/.test(block.node.meta ?? '') && session.project.strictParams;
        const result = await render(session, block.name ?? 'query.sql', `${written}\n`, `${sql}\n`, json, strict);
        if ('error' in result) {
          report(block, result.error);
          return;
        }
        analyzed++;
        if (!needed.has(block)) {
          console.log(`${DIALECT_NAME[dialect].padEnd(8)} ${block.name}: ${summary(result.text)}  (nothing imports it)`);
          return;
        }
        const slot = ((modules[dialect] ??= {})[block.name!] ??= {});
        if (slot[block.hash] === result.text) return;
        if (slot[block.hash] !== undefined) {
          errors.push(`${block.file ?? shown}:${block.line}: ${DIALECT_NAME[dialect]} ${block.name}: another page has the same statement with other types (another schema); give one of them another file name`);
          return;
        }
        slot[block.hash] = result.text;
        captured++;
        console.log(`${DIALECT_NAME[dialect].padEnd(8)} ${block.name}: ${summary(result.text)}`);
      };
      const dir = project(root, dialect);
      let url: string;
      try {
        url = dialect === 'postgres' ? await buildPostgres(schema) : dialect === 'mysql' ? await buildMysql(schema) : buildSqlite(dir, schema);
      } catch (e) {
        errors.push(`${shown}: the ${DIALECT_NAME[dialect]} ${SCHEMA_FILE} fails: ${(e as Error).message}`);
        continue;
      }
      const session = await Session.open(dir, { DATABASE_URL: url });
      try {
        const done = new Set<string>();
        for (const block of mine.filter((b) => b.schema === undefined)) {
          const key = `${block.name}\0${block.hash}`;
          if (done.has(key) && !needed.has(block)) continue;
          done.add(key);
          await analyze(session, block);
        }
      } finally {
        await session.close();
      }
      // The page's error examples in this dialect, each sent to the content mapper as a file of the
      // project, on the page's schema.
      const failing = blocks.filter((b) => b.dialect === dialect && isErrorExample(b.node));
      if (failing.length) {
        const mapper = await openMapper(dir, { DATABASE_URL: url });
        try {
          for (const block of failing) {
            if (!block.name?.endsWith('.sql')) {
              errors.push(`${block.file ?? shown}:${block.line}: an error example needs a file name ending in .sql (tab="<name>.sql")`);
              continue;
            }
            const content = `${block.node.value}\n`;
            const found = await mapper.diagnostics(block.name, content);
            // The project's own problems (no database, no engine, a bad .reylo.yaml) are the capture's, not the example's.
            const setup = found.find((d) => [4, 5, 9, 11].includes(d.code));
            if (setup || !found.length) {
              errors.push(`${block.file ?? shown}:${block.line}: ${DIALECT_NAME[dialect]} ${block.name}: ${setup ? `the content mapper could not analyze it: ${setup.messageText}` : 'the content mapper finds no error in this error example'}\n${indent(block.node.value)}`);
              continue;
            }
            ((failures[dialect] ??= {})[block.name] ??= {})[block.hash] = found.map((d) => ({ start: positionOf(content, d.start), end: positionOf(content, d.start + d.length), source: `reylo(${d.code})`, message: d.messageText }));
            console.log(`${DIALECT_NAME[dialect].padEnd(8)} ${block.name}: ${found.map((d) => `reylo${d.code}: ${d.messageText}`).join('; ')}  (error example)`);
          }
        } finally {
          await mapper.close();
        }
      }
      // A statement of an <Inferred> card that shows its own schema is analyzed against that schema,
      // built for it alone: in a scratch database made again for each card, beside the page's, and
      // a project of its own, so no session or cache of the page's schema answers for it.
      for (const [i, block] of mine.filter((b) => b.schema !== undefined).entries()) {
        const own = project(root, dialect, `${dialect}-card-${i}`);
        let cardDb: string;
        try {
          cardDb = dialect === 'postgres' ? await buildPostgres(block.schema!, cardUrl(URLS.postgres)) : dialect === 'mysql' ? await buildMysql(block.schema!, cardUrl(URLS.mysql)) : buildSqlite(own, block.schema!);
        } catch (e) {
          report(block, `the card's schema fails: ${(e as Error).message}`);
          continue;
        }
        const cardSession = await Session.open(own, { DATABASE_URL: cardDb });
        try {
          await analyze(cardSession, block);
        } finally {
          await cardSession.close();
        }
      }
    }
  }
} finally {
  rmSync(root, { recursive: true, force: true });
}

function indent(text: string) {
  return text.replace(/^/gm, '    ');
}

/** The module's parameters, row and row count on one line. */
function summary(module: string): string {
  const m = /^declare const query: Query<(.*)> & \{$/m.exec(module);
  return m ? m[1]!.replace(/, "(postgres|mysql|sqlite)"$/, '') : module;
}

for (const w of warnings) console.warn(`warning: ${w}`);
if (errors.length) {
  for (const e of errors) console.error(`error: ${e}`);
  console.error(`capture: ${errors.length} error${errors.length === 1 ? '' : 's'}; lib/sql-modules.json and lib/sql-errors.json are unchanged.`);
  process.exit(1);
}

// Sorted, so a capture that changes nothing changes nothing in git.
const sorted = Object.fromEntries(
  ORDER.filter((d) => modules[d]).map((d) => [d, Object.fromEntries(Object.entries(modules[d]!).sort(([a], [b]) => a.localeCompare(b)).map(([name, byHash]) => [name, Object.fromEntries(Object.entries(byHash).sort(([a], [b]) => a.localeCompare(b)))]))]),
);
const out = path.join(site, 'lib', 'sql-modules.json');
writeFileSync(out, JSON.stringify({ $comment: 'Written by `nub run capture` (scripts/capture-types.ts); read by lib/remark-mapper.ts. Never edit by hand.', ...sorted }, null, 2) + '\n');
const byName = <T,>(entries: Record<string, T>) => Object.fromEntries(Object.entries(entries).sort(([a], [b]) => a.localeCompare(b)));
const failed = Object.fromEntries(ORDER.filter((d) => failures[d]).map((d) => [d, byName(Object.fromEntries(Object.entries(failures[d]!).map(([name, byHash]) => [name, byName(byHash)])))]));
writeFileSync(path.join(site, 'lib', 'sql-errors.json'), JSON.stringify({ $comment: 'Written by `nub run capture` (scripts/capture-types.ts): the errors the content mapper gives each error example. Read by lib/sql-errors.ts. Never edit by hand.', ...failed }, null, 2) + '\n');
console.log(`capture: ${analyzed} statement${analyzed === 1 ? '' : 's'} analyzed on ${pages.length} page${pages.length === 1 ? '' : 's'}; ${captured} module${captured === 1 ? '' : 's'} imported → ${path.relative(site, out)}`);
