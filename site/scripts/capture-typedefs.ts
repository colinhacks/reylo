// `nub run capture-typedefs` in site/: the statement descriptor `reylo analyze` gives for each
// `<TypeDef>` card of the docs (lib/typedefs.ts), written to lib/typedefs.json, which
// lib/remark-typedefs.ts renders each card from; the build fails on a card it holds nothing for.
//
// Each card's schema is built in a scratch database of its own, made again for every card: on
// Postgres and MySQL `reylo_capture_typedefs`, on the servers REYLO_CAPTURE_POSTGRES and
// REYLO_CAPTURE_MYSQL name (whose database names must start with reylo_capture), and on SQLite a
// temporary file. The statement is analyzed in a project of its own whose `.reylo.yaml` lets a
// parameter nothing types through, so a card can show the `unknown` kind; any diagnostic fails the run.
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import postgres from 'postgres';
import mysql from 'mysql2/promise';
import { nativeBinary } from '../../src/native.ts';
import { parsePage } from '../lib/remark-mapper.ts';
import { CAPTURES, cards, keyOf, type Captures, type Card, type CardDialect } from '../lib/typedefs.ts';

const site = path.resolve(import.meta.dirname, '..');
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
  const db = new DatabaseSync(file);
  db.exec(schema);
  db.close();
  return `file:${file}`;
}

const files = readdirSync(path.join(site, 'content', 'docs')).filter((f) => f.endsWith('.mdx')).map((f) => path.join(site, 'content', 'docs', f));
const found: { file: string; card: Card }[] = files.flatMap((file) => cards(parsePage(file)).map((card) => ({ file: path.relative(site, file), card })));
const dir = mkdtempSync(path.join(tmpdir(), 'reylo-capture-typedefs-'));
writeFileSync(path.join(dir, 'package.json'), '{}');
writeFileSync(path.join(dir, '.reylo.yaml'), 'allow_untyped_params: true\n');
const captures: Captures = {};
try {
  for (const { file, card } of found) {
    if (!card.statement) continue;
    const { dialect, schema, query } = card.statement;
    const url = await build(dialect, schema ?? '', dir);
    const run = spawnSync(nativeBinary(), ['analyze', '--url', url, '--sql', query], { cwd: dir, encoding: 'utf8', env: { ...process.env, DATABASE_URL: '' } });
    const line = run.stdout.split('\n').find(Boolean);
    if (!line) fail(`${file}:${card.line}: reylo analyze printed nothing (${run.status}): ${run.stderr.trim()}`);
    const { file: _, ...descriptor } = JSON.parse(line) as { file?: string; diagnostics: { message: string }[] };
    if (descriptor.diagnostics.length && !card.errors) fail(`${file}:${card.line}: ${descriptor.diagnostics.map((d) => d.message).join('; ')}`);
    captures[keyOf(card.statement)] = { dialect, query, descriptor };
    console.log(`${file}:${card.line}: ${dialect} ${query.split('\n')[0]}`);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}
const sorted = Object.fromEntries(Object.entries(captures).sort(([a], [b]) => a.localeCompare(b)));
writeFileSync(path.join(site, 'lib', CAPTURES), `${JSON.stringify(sorted, null, 2)}\n`);
console.log(`wrote lib/${CAPTURES}: ${Object.keys(sorted).length} descriptors`);
