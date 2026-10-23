// The live card's files (lib/live/worker.ts), written to site/.live/ when the dev server or the
// build starts, and again only when what they hold changes: PGlite's WebAssembly, its data file
// renamed `.tar` (the CDN compresses a tar and leaves an `application/octet-stream` as it is), and
// a snapshot of a PGlite data directory holding the main page's Postgres schema, made here in Node
// by the same PGlite the page loads. The worker imports each with `?url`, so the build emits them
// with hashed names.
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import type { Extension } from '@electric-sql/pglite';
import type { Plugin } from 'vite';
import { pagePostgresSchema } from '../remark-mapper.ts';
import { pgJsonschemaStub, STUB_SQL } from './pg-stub.ts';

const site = path.resolve(import.meta.dirname, '../..');
const out = path.join(site, '.live');

async function writeFiles(): Promise<void> {
  const require = createRequire(path.join(site, 'package.json'));
  // The package exports no package.json; its entry point is dist/index.*.
  const pkg = path.dirname(path.dirname(require.resolve('@electric-sql/pglite')));
  const version = JSON.parse(readFileSync(path.join(pkg, 'package.json'), 'utf8')).version as string;
  const schema = pagePostgresSchema();
  const key = createHash('sha256').update(JSON.stringify([version, schema, STUB_SQL])).digest('hex');
  const keyFile = path.join(out, 'key');
  if (existsSync(keyFile) && readFileSync(keyFile, 'utf8') === key) return;
  mkdirSync(out, { recursive: true });
  copyFileSync(path.join(pkg, 'dist', 'pglite.wasm'), path.join(out, 'pglite.wasm'));
  copyFileSync(path.join(pkg, 'dist', 'pglite.data'), path.join(out, 'pglite-data.tar'));
  const { PGlite } = await import('@electric-sql/pglite');
  // pg_jsonschema exists for PGlite in no form; the stub gives the engine the catalog rows it reads.
  const pg = await PGlite.create({ extensions: { pgJsonschemaStub: pgJsonschemaStub as unknown as Extension } });
  try {
    await pg.exec(schema);
    // The template databases are two more copies of the catalog that nothing reads: without them
    // the snapshot is 25 MB where it was 41 MB.
    await pg.exec("update pg_database set datistemplate = false where datname in ('template0', 'template1')");
    await pg.exec('drop database template0');
    await pg.exec('drop database template1');
    await pg.exec('checkpoint');
    const tar = await pg.dumpDataDir('none');
    writeFileSync(path.join(out, 'postgres.tar'), new Uint8Array(await tar.arrayBuffer()));
  } finally {
    await pg.close();
  }
  writeFileSync(keyFile, key);
}

export function live(): Plugin {
  return {
    name: 'reylo-live',
    async buildStart() {
      await writeFiles();
    },
  };
}
