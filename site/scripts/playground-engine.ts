// `nub run playground-engine` in site/, which `dev` and `build` run first: puts the playground's
// engine in site/.playground/ (lib/live/engine-dir.ts), Postgres's engine core as one WebAssembly
// component transpiled by jco. No commit holds it. It comes from, in this order:
//
// - REYLO_PLAYGROUND_RELEASES, when it is set: a directory or a URL laid out as GitHub's releases
//   are, `<base>/<tag>/<asset>`, which stands in for them (a URL also serves `<base>/releases.json`,
//   in the shape of GitHub's API, and a directory lists its tags as subdirectories);
// - the checkout's own build, target/playground/postgres/, which engine/build.sh writes;
// - the GitHub release of the root package.json's version, whose asset reylo-playground-postgres.tar.gz
//   release.yml attaches beside the binaries. A version with no such release (a preview of
//   unreleased work) takes the newest release that has one, and says which.
//
// A release's tarball is checked against the SHA256SUMS of the same release before anything is
// unpacked, holds only `engine.js` and `engine.core*.wasm`, and is unpacked into a directory of its
// own first. What .playground/ holds is named in its `source` file, so a release already fetched is
// not fetched again; a release that cannot be reached, where .playground/ already holds an engine,
// leaves it as it is, with a warning, so the dev server starts offline.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { engineDir } from '../lib/live/engine-dir.ts';

const site = path.resolve(import.meta.dirname, '..');
const root = path.dirname(site);
const out = engineDir(site);
const REPOSITORY = 'colinhacks/reylo';
const ASSET = 'reylo-playground-postgres.tar.gz';
const SUMS = 'SHA256SUMS';
const BUILT = path.join(root, 'target', 'playground', 'postgres');
const FILE = /^(?:engine\.js|engine\.core\d*\.wasm)$/;

/** Where releases are read from: an asset of a tag, missing where the release or the asset is, and every release, newest first, with its assets' names. */
type Releases = { name: string; asset(tag: string, name: string): Promise<Uint8Array | undefined>; list(): Promise<{ tag: string; assets: string[] }[]> };

function sha256(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}

/** The engine's files in a directory, by name, sorted. */
function engineFiles(dir: string): string[] {
  return readdirSync(dir).filter((name) => FILE.test(name)).sort();
}

/** One hash over the engine's files in a directory, names and bytes. */
function hashDir(dir: string): string {
  const hash = createHash('sha256');
  for (const name of engineFiles(dir)) hash.update(name).update('\0').update(readFileSync(path.join(dir, name))).update('\0');
  return hash.digest('hex');
}

function holds(dir: string): boolean {
  return existsSync(path.join(dir, 'engine.js')) && engineFiles(dir).some((name) => name.endsWith('.wasm'));
}

/** What .playground/ holds, as its `source` file names it. */
function recorded(): string | undefined {
  const file = path.join(out, 'source');
  return holds(out) && existsSync(file) ? readFileSync(file, 'utf8').trim() : undefined;
}

/** Replaces .playground/ with the engine's files from `dir`, and names where they came from. */
function install(dir: string, source: string): void {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  for (const name of engineFiles(dir)) copyFileSync(path.join(dir, name), path.join(out, name));
  writeFileSync(path.join(out, 'source'), `${source}\n`);
}

/** A version's parts, for ordering tags newest first; a tag that is no version sorts last. */
function versionOf(tag: string): number[] {
  const m = /^v(\d+)\.(\d+)\.(\d+)$/.exec(tag);
  return m ? m.slice(1).map(Number) : [-1];
}

function newestFirst(a: string, b: string): number {
  const x = versionOf(a);
  const y = versionOf(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (y[i] ?? 0) - (x[i] ?? 0);
  return 0;
}

/** A release that could not be reached, as against one that was reached and refused. */
class Unreachable extends Error {}

async function get(url: string, headers: Record<string, string> = {}): Promise<Uint8Array | undefined> {
  let res: Response;
  try {
    res = await fetch(url, { headers: { 'user-agent': 'reylo-site', ...headers } });
  } catch (error) {
    throw new Unreachable(`${url}: ${(error as Error).cause ?? (error as Error).message}`);
  }
  if (res.status === 404) return undefined;
  if (!res.ok) throw new Unreachable(`${url}: ${res.status} ${res.statusText}`);
  return new Uint8Array(await res.arrayBuffer());
}

function github(): Releases {
  // The API's anonymous limit is 60 requests an hour for an address; a token, where the build has
  // one, lifts it. Only the newest-release fallback asks the API anything.
  const token = process.env.GITHUB_TOKEN;
  const auth: Record<string, string> = token ? { authorization: `Bearer ${token}` } : {};
  return {
    name: `GitHub's releases of ${REPOSITORY}`,
    asset: (tag, name) => get(`https://github.com/${REPOSITORY}/releases/download/${tag}/${name}`),
    async list() {
      const body = await get(`https://api.github.com/repos/${REPOSITORY}/releases?per_page=100`, { accept: 'application/vnd.github+json', ...auth });
      return parseList(body, `${REPOSITORY}'s releases`);
    },
  };
}

function parseList(body: Uint8Array | undefined, where: string): { tag: string; assets: string[] }[] {
  if (!body) return [];
  const releases = JSON.parse(new TextDecoder().decode(body)) as { tag_name: string; draft?: boolean; assets: { name: string }[] }[];
  if (!Array.isArray(releases)) throw new Error(`${where}: not a list of releases`);
  return releases.filter((r) => !r.draft).map((r) => ({ tag: r.tag_name, assets: r.assets.map((a) => a.name) })).sort((a, b) => newestFirst(a.tag, b.tag));
}

/** REYLO_PLAYGROUND_RELEASES: a directory of `<tag>/<asset>`, or a URL of the same with `releases.json` beside them. */
function standIn(base: string): Releases {
  if (/^https?:\/\//.test(base)) {
    const url = base.replace(/\/+$/, '');
    return {
      name: url,
      asset: (tag, name) => get(`${url}/${tag}/${name}`),
      list: async () => parseList(await get(`${url}/releases.json`), `${url}/releases.json`),
    };
  }
  const dir = path.resolve(base);
  if (!existsSync(dir) || !statSync(dir).isDirectory()) throw new Error(`REYLO_PLAYGROUND_RELEASES names ${dir}, which is not a directory or an http(s) URL`);
  return {
    name: dir,
    async asset(tag, name) {
      const file = path.join(dir, tag, name);
      return existsSync(file) ? new Uint8Array(readFileSync(file)) : undefined;
    },
    async list() {
      const tags = readdirSync(dir).filter((tag) => statSync(path.join(dir, tag)).isDirectory()).sort(newestFirst);
      return tags.map((tag) => ({ tag, assets: readdirSync(path.join(dir, tag)) }));
    },
  };
}

/** The tarball's files, refused unless SHA256SUMS of the same release names its hash and it holds only the engine's files. */
async function unpack(releases: Releases, tag: string, tarball: Uint8Array): Promise<string> {
  const sums = await releases.asset(tag, SUMS);
  if (!sums) throw new Error(`${releases.name}: ${tag} has ${ASSET} and no ${SUMS} to check it against`);
  const line = new TextDecoder().decode(sums).split('\n').map((l) => l.trim().split(/\s+\*?/)).find((parts) => parts[1] === ASSET);
  if (!line) throw new Error(`${releases.name}: ${tag}'s ${SUMS} names no ${ASSET}`);
  const actual = sha256(tarball);
  if (line[0] !== actual) throw new Error(`${releases.name}: ${tag}'s ${ASSET} has the SHA-256 ${actual}, and its ${SUMS} says ${line[0]}`);
  const temp = mkdtempSync(path.join(tmpdir(), 'reylo-playground-'));
  const archive = path.join(temp, ASSET);
  const files = path.join(temp, 'files');
  writeFileSync(archive, tarball);
  mkdirSync(files);
  const names = execFileSync('tar', ['-tzf', archive], { encoding: 'utf8' }).split('\n').filter(Boolean).map((n) => n.replace(/^\.\//, '')).filter((n) => n !== '' && n !== '.');
  const stray = names.filter((n) => !FILE.test(n));
  if (stray.length) throw new Error(`${releases.name}: ${tag}'s ${ASSET} holds ${stray.join(', ')}, beside the engine's files`);
  execFileSync('tar', ['-xzf', archive, '-C', files]);
  for (const name of readdirSync(files)) {
    if (!FILE.test(name) || !lstatSync(path.join(files, name)).isFile()) throw new Error(`${releases.name}: ${tag}'s ${ASSET} unpacked ${name}, which is not one of the engine's files`);
  }
  if (!holds(files)) throw new Error(`${releases.name}: ${tag}'s ${ASSET} holds no engine.js and engine.core*.wasm`);
  return files;
}

async function fromReleases(releases: Releases, version: string): Promise<void> {
  const wanted = `v${version}`;
  if (recorded() === `release ${wanted}`) {
    console.log(`site/.playground: the playground's engine of ${wanted}, already fetched`);
    return;
  }
  let tag = wanted;
  let tarball = await releases.asset(tag, ASSET);
  if (!tarball) {
    const found = (await releases.list()).find((r) => r.assets.includes(ASSET) && r.assets.includes(SUMS));
    if (!found) {
      throw new Error(`${releases.name}: no release carries ${ASSET}, the playground's engine, for ${wanted} or any other version. Build it in this checkout with engine/build.sh (it needs rustup's wasm32-wasip2 target, wasi-sdk 34 and jco: AGENTS.md's Rust section), which writes target/playground/postgres/, then run this again.`);
    }
    if (recorded() === `release ${found.tag}`) {
      console.warn(`site/.playground: ${wanted} has no release with the playground's engine (a preview of unreleased work), so it keeps ${found.tag}'s, the newest release's that has one`);
      return;
    }
    tag = found.tag;
    tarball = await releases.asset(tag, ASSET);
    if (!tarball) throw new Error(`${releases.name}: ${tag} lists ${ASSET}, and fetching it found nothing`);
    console.warn(`site/.playground: ${wanted} has no release with the playground's engine (a preview of unreleased work), so it takes ${tag}'s, the newest release's that has one`);
  }
  const files = await unpack(releases, tag, tarball);
  try {
    install(files, `release ${tag}`);
  } finally {
    rmSync(path.dirname(files), { recursive: true, force: true });
  }
  console.log(`site/.playground: the playground's engine of ${tag}, from ${releases.name} (${Math.round(tarball.length / 1024)} KB)`);
}

async function main(): Promise<void> {
  const version = (JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')) as { version: string }).version;
  const base = process.env.REYLO_PLAYGROUND_RELEASES;
  if (!base && holds(BUILT)) {
    const source = `build ${hashDir(BUILT)}`;
    if (recorded() !== source) install(BUILT, source);
    console.log(`site/.playground: the playground's engine from target/playground/postgres/, this checkout's build`);
    return;
  }
  const releases = base ? standIn(base) : github();
  try {
    await fromReleases(releases, version);
  } catch (error) {
    if (!(error instanceof Unreachable) || !holds(out)) throw error;
    console.warn(`site/.playground: kept the engine it holds (${recorded() ?? 'from an unknown source'}), since fetching ${version}'s failed: ${(error as Error).message}`);
  }
}

main().catch((error: Error) => {
  console.error(`scripts/playground-engine.ts: ${error.message}`);
  process.exit(1);
});
