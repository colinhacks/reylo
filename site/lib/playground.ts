// The local playground (app/routes/playground.tsx): an Inference card for any statement, drawn as
// the page draws one, from the types Reylo infers for it on the page's schema. The dev server alone
// serves it (lib/playground-plugin.ts), and only on a machine with scratch databases: it builds the
// page's schema, or the one the address gives, in Postgres and MySQL databases named
// reylo_capture_playground… (REYLO_CAPTURE_POSTGRES and REYLO_CAPTURE_MYSQL name the servers, as
// for `nub run capture`) and a SQLite file, has the loader's own Session analyze the statement on
// each, and compiles the card's rows with the page's MDX options, so that its code is highlighted
// as every other card's. Nothing here reaches the built site.
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { compile } from '@mdx-js/mdx';
import mysql from 'mysql2/promise';
import postgres from 'postgres';
import { Session } from '../../src/analyze.ts';
import type { Analysis } from '../../src/engine.ts';
import { ident, renderParamType, renderType } from '../../src/render.ts';
import { schemaType } from '../scripts/schema-type.ts';
import { mdxOptions } from './mdx-options.ts';
import { pageFiles } from './page-files.ts';
import { collectFiles, DIALECT_NAME, INFERRED_LAYOUT, SCHEMA_FILE, type Dialect } from './remark-mapper.ts';
import { formatType, resultType } from '../../src/type-format.ts';

const site = path.resolve(import.meta.dirname, '..');
const ORDER: Dialect[] = ['postgres', 'mysql', 'sqlite'];
const SERVERS: Record<Exclude<Dialect, 'sqlite'>, string> = {
  postgres: process.env.REYLO_CAPTURE_POSTGRES ?? 'postgres://postgres@localhost:54329/reylo_capture',
  mysql: process.env.REYLO_CAPTURE_MYSQL ?? 'mysql://root@127.0.0.1:54336/reylo_capture',
};
// The driver each scratch project names, whose defaults the types follow, as in `nub run capture`.
const DRIVER: Record<Dialect, string | undefined> = { postgres: 'postgres', mysql: 'mysql2', sqlite: undefined };
const TYPE_WIDTH = { side: 30, rows: 64 } as const;

/** What the playground asks for: the statement, the dialects to try it on, and a schema of the card's own. */
export type Request = { sql: string; dialects?: Dialect[]; schema?: string };
/** The card's compiled MDX (a function body), and each dialect's error where the statement fails there. */
export type Card = { code?: string; errors: Partial<Record<Dialect, string>> };

/** The main page's schema, per dialect: its `schema.sql` blocks, as `nub run capture` builds them. */
function pageSchemas(): Partial<Record<Dialect, string>> {
  const blocks = collectFiles(pageFiles(site), site);
  const out: Partial<Record<Dialect, string>> = {};
  for (const d of ORDER) {
    const own = blocks.filter((b) => b.name === SCHEMA_FILE && b.dialect === d);
    if (own.length) out[d] = own.map((b) => b.node.value).join('\n\n');
  }
  return out;
}

/** A database the playground drops and makes again: its name starts with reylo_capture, as capture's do. */
function scratchUrl(server: string, suffix: string): { url: string; db: string; admin: URL } {
  const u = new URL(server);
  const db = `reylo_capture_playground${suffix}`;
  u.pathname = `/${db}`;
  return { url: u.href, db, admin: new URL(server) };
}

async function buildPostgres(schema: string, suffix: string): Promise<string> {
  const { url, db, admin } = scratchUrl(SERVERS.postgres, suffix);
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

async function buildMysql(schema: string, suffix: string): Promise<string> {
  const { url, db, admin } = scratchUrl(SERVERS.mysql, suffix);
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

const root = mkdtempSync(path.join(tmpdir(), 'reylo-playground-'));

/** A project per dialect and schema, as an app using Reylo has, its driver resolving from the site's node_modules. */
function project(dialect: Dialect, suffix: string): string {
  const dir = path.join(root, `${dialect}${suffix}`);
  mkdirSync(dir, { recursive: true });
  const driver = DRIVER[dialect];
  writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name: `reylo-playground-${dialect}`, private: true, type: 'module', dependencies: driver ? { [driver]: '*' } : {} }));
  symlinkSync(path.join(site, 'node_modules'), path.join(dir, 'node_modules'), 'dir');
  return dir;
}

/** One session per dialect and schema, kept for the server's life: building a schema is the slow part. */
const sessions = new Map<string, Promise<Session>>();

function session(dialect: Dialect, schema: string): Promise<Session> {
  const suffix = `_${createHash('sha256').update(schema).digest('hex').slice(0, 10)}`;
  const key = `${dialect}${suffix}`;
  let found = sessions.get(key);
  if (!found) {
    found = (async () => {
      const dir = project(dialect, suffix);
      let url: string;
      if (dialect === 'postgres') url = await buildPostgres(schema, suffix);
      else if (dialect === 'mysql') url = await buildMysql(schema, suffix);
      else {
        const db = new DatabaseSync(path.join(dir, 'app.db'));
        try {
          db.exec(schema);
        } finally {
          db.close();
        }
        url = 'file:./app.db';
      }
      return Session.open(dir, { DATABASE_URL: url });
    })();
    // A schema that fails to build is tried again on the next request.
    found.catch(() => sessions.delete(key));
    sessions.set(key, found);
  }
  return found;
}

// ---- The syntax the engine does not build yet, as scripts/capture-types.ts compiles it ----

const NAME = String.raw`:[A-Za-z_]\w*`;
const SCHEMA_STRING = String.raw`'((?:[^']|'')*)'`;

function jsonParams(sql: string): { name: string; schema: unknown }[] {
  return [...sql.matchAll(new RegExp(String.raw`\(?(${NAME})\)?::jsonb?\(${SCHEMA_STRING}\)`, 'g'))].map((m) => ({ name: m[1]!.slice(1), schema: JSON.parse(m[2]!.replace(/''/g, "'")) }));
}

function compileSql(sql: string, dialect: Dialect): string {
  if (dialect === 'postgres') return sql.replace(new RegExp(String.raw`::(jsonb?)\(${SCHEMA_STRING}\)`, 'g'), '::$1');
  const json = new RegExp(String.raw`\(?(${NAME})\)?::json\(${SCHEMA_STRING}\)`, 'g');
  return sql.replace(json, dialect === 'mysql' ? 'cast($1 as json)' : '$1');
}

/** The statement's parameters, row and cardinality, as scripts/capture-types.ts renders a module, or the error the mapper would show. */
async function typesOf(dialect: Dialect, schema: string, written: string): Promise<{ params?: string; row: string; cardinality: Analysis['cardinality'] } | { error: string }> {
  const s = await session(dialect, schema);
  const json = new Map(jsonParams(written).map((p) => [p.name, schemaType(p.schema as never, dialect, `:${p.name}`)]));
  const outcome = await s.analyze(`${compileSql(written, dialect)}\n`);
  if (!outcome.ok) return { error: outcome.message };
  const { analysis } = outcome;
  if (analysis.diagnostics.length) return { error: analysis.diagnostics.map((d) => d.message).join('\n') };
  const untyped = analysis.params.filter((q) => q.type.kind === 'unknown' && !json.has(q.name));
  if (untyped.length && s.project.strictParams) return { error: `Nothing in the statement types ${untyped.map((q) => `:${q.name}`).join(', ')}; give it a cast.` };
  const params = analysis.params.map((q) => {
    const rendered = json.get(q.name) ?? (q.type.kind === 'unknown' ? 'unknown' : `${renderParamType(q.type, dialect)}${q.nullable ? ' | null' : ''}`);
    return `${ident(q.name)}: ${rendered}`;
  });
  const columns = analysis.columns.map((c) => `${ident(c.name)}: ${renderType(c.type, false, dialect)}${c.nullable ? ' | null' : ''}`);
  return { params: params.length ? `{ ${params.join('; ')} }` : undefined, row: columns.length ? `{ ${columns.join('; ')} }` : 'never', cardinality: analysis.cardinality };
}

/** A fenced block that no line of its text can close. */
function fence(lang: string, text: string): string {
  const longest = Math.max(2, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  const ticks = '`'.repeat(longest + 1);
  return `${ticks}${lang}\n${text}\n${ticks}`;
}

/** The card for `request`: one panel per dialect the statement types on, laid out as lib/remark-mapper.ts lays out an `<Inferred>` card. */
export async function card(request: Request): Promise<Card> {
  const written = request.sql.trim();
  const own = request.schema?.trim();
  const schemas = own ? Object.fromEntries(ORDER.map((d) => [d, own])) : pageSchemas();
  const errors: Card['errors'] = {};
  const panels: string[] = [];
  const width = TYPE_WIDTH[INFERRED_LAYOUT];
  for (const dialect of request.dialects?.length ? ORDER.filter((d) => request.dialects!.includes(d)) : ORDER) {
    const schema = schemas[dialect];
    if (schema === undefined) {
      errors[dialect] = `The page has no ${DIALECT_NAME[dialect]} schema.`;
      continue;
    }
    let types: Awaited<ReturnType<typeof typesOf>>;
    try {
      types = await typesOf(dialect, schema, written);
    } catch (e) {
      types = { error: (e as Error).message };
    }
    if ('error' in types) {
      errors[dialect] = types.error;
      continue;
    }
    const rows = [];
    if (own) rows.push(`<InferredRow label="Schema">\n\n${fence('sql', own)}\n\n</InferredRow>`);
    rows.push(`<InferredRow label="Query">\n\n${fence('sql', written)}\n\n</InferredRow>`);
    const typeRows = [];
    typeRows.push(`<InferredRow label="Params">\n\n${fence('ts', types.params ? formatType(types.params, width) : 'void')}\n\n</InferredRow>`);
    typeRows.push(`<InferredRow label="Returns">\n\n${fence('ts', formatType(resultType(types.row, types.cardinality), width))}\n\n</InferredRow>`);
    rows.push(`<InferredTypes>\n${typeRows.join('\n')}\n</InferredTypes>`);
    panels.push(`<Dialect value="${DIALECT_NAME[dialect]}">\n${rows.join('\n')}\n</Dialect>`);
  }
  if (!panels.length) return { errors };
  const shown = ORDER.filter((d) => errors[d] === undefined && (request.dialects?.length ? request.dialects.includes(d) : true)).map((d) => DIALECT_NAME[d]);
  // `<PlaygroundCard>` is the page's `<Inferred>` under another name (app/routes/playground.tsx), since
  // lib/remark-mapper.ts fills every `<Inferred>` from the captured modules.
  const mdx = `<PlaygroundCard dialects="${shown.join(',')}" layout="${INFERRED_LAYOUT}">\n<DialectTabs follow>\n${panels.join('\n')}\n</DialectTabs>\n</PlaygroundCard>\n`;
  const options = await mdxOptions('runtime');
  const file = await compile({ value: mdx, path: path.join(root, 'playground.mdx') }, { ...options, outputFormat: 'function-body' });
  return { code: String(file), errors };
}
