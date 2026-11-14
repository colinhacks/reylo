// `nub run capture-typedefs` in site/: what `reylo analyze` prints for each `<Analyzed>` card of the
// docs (lib/typedefs.ts), as YAML and as JSON, written to lib/typedefs.json, which
// lib/remark-typedefs.ts renders each card from; the build fails on a card it holds nothing for.
//
// Each card's schema is built in a scratch database of its own, made again for every card: on
// Postgres and MySQL `reylo_capture_typedefs`, on the servers REYLO_CAPTURE_POSTGRES and
// REYLO_CAPTURE_MYSQL name (whose database names must start with reylo_capture), and on SQLite and
// DuckDB a temporary file. The statement is written to the card's file in a project of its own, and
// analyzed there as the docs show the command run; any diagnostic fails the run. `reylo analyze`
// opens no DuckDB database, so a DuckDB card is analyzed by the root module's `analyze()`, through
// lab/drivers' @duckdb/node-api linked into the project, as an app's install holds it.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import postgres from 'postgres';
import mysql from 'mysql2/promise';
import { nativeBinary } from '../../src/native.ts';
import { analyze } from '../../src/index.ts';
import { parsePage } from '../lib/remark-mapper.ts';
import { analyzedCards, CAPTURES, keyOf, type AnalyzedCard, type Captures, type CardDialect } from '../lib/typedefs.ts';

const site = path.resolve(import.meta.dirname, '..');
const DUCKDB = path.resolve(site, '..', 'lab', 'drivers', 'node_modules', '@duckdb', 'node-api');
const DB = 'reylo_capture_typedefs';
const SERVERS = {
  postgres: process.env.REYLO_CAPTURE_POSTGRES ?? 'postgres://postgres@localhost:54329/reylo_capture',
  mysql: process.env.REYLO_CAPTURE_MYSQL ?? 'mysql://root@127.0.0.1:54336/reylo_capture',
};

function fail(message: string): never {
  console.error(`capture-typedefs: ${message}`);
  process.exit(1);
}

/** The server's URL with the scratch database in it, checked to be a capture's own. */
function scratch(dialect: 'postgres' | 'mysql'): URL {
  const u = new URL(SERVERS[dialect]);
  if (!decodeURIComponent(u.pathname.slice(1)).startsWith('reylo_capture')) fail(`${u.href} names no reylo_capture database; the capture drops and makes its databases again, so it only runs where the name says it may`);
  u.pathname = `/${DB}`;
  return u;
}

async function build(dialect: CardDialect, schema: string, dir: string): Promise<string> {
  if (dialect === 'postgres') {
    const url = scratch('postgres');
    const admin = new URL(url);
    admin.pathname = '/postgres';
    const server = postgres(admin.href, { max: 1, onnotice: () => {} });
    try {
      await server.unsafe(`drop database if exists "${DB}" with (force)`);
      await server.unsafe(`create database "${DB}"`);
    } finally {
      await server.end();
    }
    const sql = postgres(url.href, { max: 1, onnotice: () => {} });
    try {
      if (schema) await sql.unsafe(schema).simple();
    } finally {
      await sql.end();
    }
    return url.href;
  }
  if (dialect === 'mysql') {
    const url = scratch('mysql');
    const admin = new URL(url);
    admin.pathname = '/';
    const conn = await mysql.createConnection({ uri: admin.href, multipleStatements: true });
    try {
      await conn.query(`drop database if exists \`${DB}\``);
      await conn.query(`create database \`${DB}\``);
      await conn.query(`use \`${DB}\``);
      if (schema) await conn.query(schema);
    } finally {
      await conn.end();
    }
    return url.href;
  }
  const file = path.join(dir, `card-${Date.now()}-${Math.random().toString(36).slice(2)}.db`);
  if (dialect === 'duckdb') {
    if (!existsSync(DUCKDB)) fail('a DuckDB card needs lab/drivers installed, whose @duckdb/node-api it reads');
    const { DuckDBInstance } = createRequire(path.join(DUCKDB, 'package.json'))('@duckdb/node-api');
    const instance = await DuckDBInstance.create(file);
    const connection = await instance.connect();
    if (schema) await connection.run(schema);
    connection.closeSync();
    instance.closeSync();
    return `duckdb:${file}`;
  }
  const db = new DatabaseSync(file);
  db.exec(schema);
  db.close();
  return `file:${file}`;
}

const files = readdirSync(path.join(site, 'content', 'docs')).filter((f) => f.endsWith('.mdx')).map((f) => path.join(site, 'content', 'docs', f));
const found: { file: string; card: AnalyzedCard }[] = files.flatMap((file) => analyzedCards(parsePage(file)).map((card) => ({ file: path.relative(site, file), card })));
const dir = mkdtempSync(path.join(tmpdir(), 'reylo-capture-typedefs-'));
writeFileSync(path.join(dir, 'package.json'), '{}');
// The project's install, as an app's holds it: reylo, this checkout, and DuckDB's driver.
mkdirSync(path.join(dir, 'node_modules', '@duckdb'), { recursive: true });
symlinkSync(path.resolve(site, '..'), path.join(dir, 'node_modules', 'reylo'));
if (existsSync(DUCKDB)) symlinkSync(DUCKDB, path.join(dir, 'node_modules', '@duckdb', 'node-api'));
const captures: Captures = {};
try {
  for (const { file, card } of found) {
    const url = await build(card.dialect, card.schema ?? '', dir);
    mkdirSync(path.dirname(path.join(dir, card.file)), { recursive: true });
    writeFileSync(path.join(dir, card.file), `${card.query.trimEnd()}\n`);
    const run = (format: 'yaml' | 'json') => {
      const out = spawnSync(nativeBinary(), ['analyze', card.file, '--url', url, '--format', format], { cwd: dir, encoding: 'utf8', env: { ...process.env, DATABASE_URL: '' } });
      if (out.status !== 0 || !out.stdout.trim()) fail(`${file}:${card.line}: reylo analyze --format ${format} exited ${out.status}: ${out.stderr.trim()}`);
      return out.stdout;
    };
    if (card.dialect === 'duckdb') {
      const cwd = process.cwd();
      process.chdir(dir);
      try {
        const descriptor = await analyze({ url, sql: card.query });
        if (descriptor.diagnostics.length) fail(`${file}:${card.line}: ${descriptor.diagnostics.map((d) => d.message).join('; ')}`);
        captures[keyOf(card)] = { dialect: card.dialect, file: card.file, query: card.query, yaml: '', json: `${JSON.stringify({ file: card.file, ...descriptor })}\n` };
      } finally {
        process.chdir(cwd);
      }
    } else captures[keyOf(card)] = { dialect: card.dialect, file: card.file, query: card.query, yaml: run('yaml'), json: run('json') };
    console.log(`${file}:${card.line}: ${card.dialect} ${card.file}`);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}
const sorted = Object.fromEntries(Object.entries(captures).sort(([a], [b]) => a.localeCompare(b)));
writeFileSync(path.join(site, 'lib', CAPTURES), `${JSON.stringify(sorted, null, 2)}\n`);
console.log(`wrote lib/${CAPTURES}: ${Object.keys(sorted).length} outputs`);
