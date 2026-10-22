import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Engine } from "../src/engine.ts";

// Each committed engine.wasm must be the build of the engine sources beside it. An artifact reports
// its fingerprint on ping: core's source hash combined with its database crate's
// (engine/core/src/fingerprint.rs), each the hash a crate's build script takes of its own sources
// (engine/build-support/src/lib.rs). This takes both hashes again, the same way, and compares, so a
// change to an engine's sources whose artifact was not rebuilt fails here, with no Rust toolchain.

const ENGINE = fileURLToPath(new URL("../engine", import.meta.url));
const PACKAGES = fileURLToPath(new URL("../packages", import.meta.url));

/** FNV-1a, 64-bit, over `bytes`, from `hash`; a carriage return is skipped where `crlf` is set, as the build script does. */
function fnv(hash: bigint, bytes: Uint8Array, crlf: boolean): bigint {
  for (const byte of bytes) {
    if (crlf && byte === 13) continue;
    hash = ((hash ^ BigInt(byte)) * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash;
}

/** What a crate's build script hashes besides `Cargo.toml`: the entries it hands `reylo_engine_build::emit`. */
function entries(crate: string): string[] {
  const script = readFileSync(join(crate, "build.rs"), "utf8");
  const list = /reylo_engine_build::emit\(\s*"\w+"\s*,\s*&\[([^\]]*)\]/.exec(script)?.[1];
  assert.ok(list !== undefined, `${crate}/build.rs hands reylo_engine_build::emit no list of entries`);
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

/** `reylo_engine_core::fingerprint::combine` of core's hash and the database crate's: FNV-1a over their little-endian bytes. */
function fingerprint(database: string): string {
  const parts = Buffer.alloc(16);
  parts.writeBigUInt64LE(sourceHash(join(ENGINE, "core")), 0);
  parts.writeBigUInt64LE(sourceHash(join(ENGINE, database)), 8);
  return fnv(0xcbf29ce484222325n, parts, false).toString(16).padStart(16, "0");
}

for (const database of ["postgres", "sqlite", "mysql"]) {
  test(`packages/${database}/engine.wasm is the build of the engine's sources`, async () => {
    const engine = await Engine.load(join(PACKAGES, database, "engine.wasm"));
    const ping = engine.call({ op: "ping" }) as { fingerprint: string };
    assert.equal(ping.fingerprint, fingerprint(database), `packages/${database}/engine.wasm was built from other sources than engine/core and engine/${database}: rebuild it with engine/build.sh ${database} and commit it`);
  });
}
