import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { engine } from "../src/engine.ts";

// The engine the tests run must be the build of the sources beside it. Its ping reports each
// database's fingerprint, core's source hash combined with its database crate's
// (engine/core/src/fingerprint.rs), each the hash a crate's build script takes of its sources
// (engine/build-support/src/lib.rs). This takes them again, the same way, and compares, so that a
// change to an engine's sources that the binary was not rebuilt from, a checkout's
// target/release/reylo above all, which is found before target/debug/reylo, fails here.

const ENGINE = fileURLToPath(new URL("../engine", import.meta.url));

/** FNV-1a, 64-bit, over `bytes`, from `hash`; a carriage return is skipped where `crlf` is set, as the build script does. */
function fnv(hash: bigint, bytes: Uint8Array, crlf: boolean): bigint {
  for (const byte of bytes) {
    if (crlf && byte === 13) continue;
    hash = ((hash ^ BigInt(byte)) * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash;
}

/** What a crate's build script hashes besides `Cargo.toml`: the entries it hands `reylo_engine_build::emit`, or its own `source_hash`, which the copy of `sqlparser` carries so that it names no crate of the workspace. */
function entries(crate: string): string[] {
  const script = readFileSync(join(crate, "build.rs"), "utf8");
  const list = /(?:reylo_engine_build::emit\(\s*"\w+"|source_hash\(\s*&root)\s*,\s*&\[([^\]]*)\]/.exec(script)?.[1];
  assert.ok(list !== undefined, `${crate}/build.rs hands neither reylo_engine_build::emit nor its own source_hash a list of entries`);
  return [...list.matchAll(/"([^"]+)"/g)].map((m) => m[1]!);
}

/** Every file under `dir` but a dotfile or dot-directory, as paths relative to the crate. */
function walk(crate: string, dir: string, out: string[]): void {
  for (const name of readdirSync(join(crate, dir))) {
    if (name.startsWith(".")) continue;
    const path = `${dir}/${name}`;
    const stat = statSync(join(crate, path));
    if (stat.isDirectory()) walk(crate, path, out);
    else if (stat.isFile()) out.push(path);
  }
}

/** Paths in the order Rust sorts a `PathBuf`: component by component, each by its bytes. */
function byComponents(a: string, b: string): number {
  const x = a.split("/"), y = b.split("/");
  for (let i = 0; i < Math.min(x.length, y.length); i++) {
    const order = Buffer.compare(Buffer.from(x[i]!), Buffer.from(y[i]!));
    if (order) return order;
  }
  return x.length - y.length;
}

/** `reylo_engine_build::source_hash` over the crate at `crate`. */
function sourceHash(crate: string): bigint {
  const files = ["Cargo.toml"];
  for (const entry of entries(crate)) {
    if (statSync(join(crate, entry), { throwIfNoEntry: false })?.isFile()) {
      files.push(entry);
      continue;
    }
    const found: string[] = [];
    walk(crate, entry, found);
    files.push(...found.sort(byComponents));
  }
  let hash = 0xcbf29ce484222325n;
  for (const file of files) {
    hash = fnv(hash, Buffer.from(file), true);
    hash = fnv(hash, Buffer.of(0), true);
    hash = fnv(hash, readFileSync(join(crate, file)), true);
    hash = fnv(hash, Buffer.of(0), true);
  }
  return hash;
}

/** The crates of its own a database's engine builds on, whose hashes its fingerprint combines after its own: MySQL's copy of `sqlparser`. */
const BUILDS_ON: Record<string, string[]> = { mysql: ["mysql/vendor/sqlparser"] };

/** `reylo_engine_core::fingerprint::combine` of core's hash, the database crate's and each crate it builds on: FNV-1a over their little-endian bytes. */
function fingerprint(database: string): string {
  const crates = ["core", database, ...(BUILDS_ON[database] ?? [])];
  const parts = Buffer.alloc(8 * crates.length);
  crates.forEach((crate, i) => parts.writeBigUInt64LE(sourceHash(join(ENGINE, crate)), 8 * i));
  return fnv(0xcbf29ce484222325n, parts, false).toString(16).padStart(16, "0");
}

for (const database of ["postgres", "sqlite", "mysql"] as const) {
  test(`the ${database} engine is the build of the engine's sources`, async () => {
    const { binary, fingerprint: built } = await engine(database);
    assert.equal(built, fingerprint(database), `${binary} was built from other sources than engine/core and engine/${database}: build it again with cargo build --release -p reylo-cli`);
  });
}
