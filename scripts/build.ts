// Builds every publishable package into dist/, one directory each, published with
// `npm publish dist/<name>`:
// - dist/reylo: the run time, the entry points a project names and the build-time half they run
//   (tsconfig.build.json), compiled to JavaScript with their declarations, the README, `reylo play`'s
//   page in play/ (built from site/), and a package.json whose exports and content mapper name the
//   compiled files. Node strips no types under node_modules, so an installed copy that ships
//   TypeScript fails to load in the mapper, the bundler adapters and at run time. It carries no
//   engine: the engine is the native `reylo` binary, one package per platform, which this
//   package.json lists as optional dependencies at reylo's exact version, so that a package manager
//   installs the one whose `os`, `cpu` and `libc` match the machine and skips the rest.
// - dist/engine-<platform>: `@reylosql/engine-<platform>` (the table is npm/platforms.json), at
//   reylo's version, holding `bin/reylo` (`bin/reylo.exe` on Windows), its package.json and README.
//   It declares no `bin` and runs no install script: the binary is packed with its execute bit,
//   which npm, pnpm and Bun keep when they unpack it, and reylo's entry points start it by its path
//   (src/native.ts).
//
//   nub scripts/build.ts                     this machine's engine, built here with cargo
//   nub scripts/build.ts --binary <file>     this machine's engine, from a binary already built
//   nub scripts/build.ts --binaries <dir>    every platform's engine, from <dir>/engine-<platform>/reylo[.exe]
//
// The repository's own package.json keeps pointing at the sources, so the tests and the labs run
// them without a build, and names no platform package: a checkout runs the binary it built itself
// (target/release/reylo), which an installed platform package would otherwise shadow. It is
// private, so that it cannot be published by mistake.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync, existsSync } from "node:fs";
import { join, relative } from "node:path";
import { parseArgs } from "node:util";
import { transformSync } from "esbuild";
import { PLATFORM_PACKAGES } from "../src/native.ts";

// How the release builds the binary, here and in .github/scripts/build-binary.sh, which sets the
// same: stripped of its symbol table, which is a tenth of it and which no user reads. SQLite's flags
// come from .cargo/config.toml, which every cargo build in the workspace reads.
const RELEASE_ENV = {
  CARGO_PROFILE_RELEASE_STRIP: "symbols",
};

type Platform = { platform: string; os: string; cpu: string; libc?: "glibc" | "musl"; target: string; runner: string; name: string };

const root = join(import.meta.dirname, "..");
const dist = join(root, "dist");
const js = (path: string) => path.replace(/\.ts$/, ".js");
const PLATFORMS: Platform[] = JSON.parse(readFileSync(join(root, "npm", "platforms.json"), "utf8"));
// The run time finds this machine's package by its own table (src/native.ts), which names the same platforms.
const table = Object.entries(PLATFORM_PACKAGES).map(([key, name]) => `${key} ${name}`).sort();
const listed = PLATFORMS.map((p) => `${p.platform} @reylosql/engine-${p.platform}`).sort();
if (table.join() !== listed.join()) throw new Error(`src/native.ts's PLATFORM_PACKAGES (${table.join(", ")}) and npm/platforms.json (${listed.join(", ")}) name different platforms`);
const { values: args } = parseArgs({ options: { binary: { type: "string" }, binaries: { type: "string" } } });
if (args.binary && args.binaries) throw new Error("--binary names this machine's engine and --binaries every platform's: pass one");

const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
// The binary prints and answers with the Cargo workspace's version, which is reylo's: the two move together.
const cargoVersion = readFileSync(join(root, "Cargo.toml"), "utf8").match(/^\[workspace\.package\]\s*\nversion = "([^"]+)"/m)?.[1];
if (cargoVersion !== pkg.version) throw new Error(`Cargo.toml: the workspace's version is ${cargoVersion}, and reylo is ${pkg.version}; they move together`);
if (pkg.optionalDependencies && Object.keys(pkg.optionalDependencies).some((name) => name.startsWith("@reylosql/"))) {
  throw new Error("package.json names a platform package: the build writes them into dist/reylo/package.json, and a checkout runs its own binary");
}

rmSync(dist, { recursive: true, force: true });
const reylo = join(dist, "reylo");
// TypeScript's own launcher, run by this Node: node_modules/.bin holds a shell script, which Windows cannot start.
execFileSync(process.execPath, [join(root, "node_modules/typescript/bin/tsc"), "-p", join(root, "tsconfig.build.json")], { stdio: "inherit" });
// The comments stay in the sources and in the declarations, where an editor shows them. The
// JavaScript ships without them, since they are about a third of its size.
for (const file of readdirSync(reylo, { recursive: true, encoding: "utf8" }).filter((f) => f.endsWith(".js"))) {
  const path = join(reylo, file);
  writeFileSync(path, transformSync(readFileSync(path, "utf8"), { loader: "js", legalComments: "inline" }).code);
}
copyFileSync(join(root, "README.md"), join(reylo, "README.md"));
copyFileSync(join(root, "LICENSE"), join(reylo, "LICENSE"));

const { private: _private, scripts: _scripts, devDependencies: _dev, bin, exports, typescript, ...rest } = pkg;
const built = {
  ...rest,
  // A bin path without its "./", as npm writes it: npm rewrites `./src/cli.js` to `src/cli.js` at
  // publish and warns that the script "was invalid and removed", which it was not (0.0.1's stage).
  bin: Object.fromEntries(Object.entries(bin as Record<string, string>).map(([name, path]) => [name, js(path).replace(/^\.\//, "")])),
  exports: Object.fromEntries(Object.entries(exports as Record<string, string>).map(([key, path]) => [key, { types: path.replace(/\.ts$/, ".d.ts"), default: js(path) }])),
  typescript: { ...typescript, contentMapper: { ...typescript.contentMapper, exec: (typescript.contentMapper.exec as string[]).map(js) } },
  // Every platform's engine at reylo's own version: reylo refuses a binary of another.
  optionalDependencies: Object.fromEntries(PLATFORMS.map((p) => [`@reylosql/engine-${p.platform}`, pkg.version])),
};
writeFileSync(join(reylo, "package.json"), JSON.stringify(built, null, 2) + "\n");

// Nothing TypeScript ships but declarations, and no engine ships inside reylo: neither a
// WebAssembly component nor a native program.
// With "/" between the parts, as the package.json names them, on Windows too.
const files = readdirSync(reylo, { recursive: true, encoding: "utf8" }).filter((f) => statSync(join(reylo, f)).isFile()).map((f) => f.replaceAll("\\", "/"));
const source = files.filter((f) => /\.(c|m)?ts$/.test(f) && !/\.d\.(c|m)?ts$/.test(f));
if (source.length) throw new Error(`dist/reylo holds TypeScript sources: ${source.join(", ")}`);
const engines = files.filter((f) => f.endsWith(".wasm") || executableFormat(readFileSync(join(reylo, f)).subarray(0, 4096)));
if (engines.length) throw new Error(`dist/reylo holds an engine: ${engines.join(", ")}`);
// Every file the package.json names: the bin, each export with its declarations, and the content mapper.
const named = [...Object.values(built.bin as Record<string, string>), ...Object.values(built.exports as Record<string, { types: string; default: string }>).flatMap((e) => [e.types, e.default]), ...(built.typescript.contentMapper.exec as string[]).slice(1)];
const unnamed = named.map((f) => f.replace(/^\.\//, "")).filter((f) => !files.includes(f));
if (unnamed.length) throw new Error(`dist/reylo/package.json names files the build did not write: ${unnamed.join(", ")}`);
// The adapters load the build-time half with `require` (src/tooling-loader.ts), which refuses a
// module graph with a top-level await in it: it is loaded here as they load it.
execFileSync(process.execPath, ["-e", "require(process.argv[1])", join(reylo, "src", "tooling.js")], { stdio: "inherit" });
console.log(`dist/reylo: ${files.length} files, package ${built.name}@${built.version}`);

// `reylo play`'s page (src/play.ts), built from the docs site's playground with the site's own
// toolchain into dist/reylo/play/, which needs the site's install (\`nub install\` in site/).
const site = join(root, "site");
if (!existsSync(join(site, "node_modules", "vite"))) throw new Error("site/ is not installed, and reylo play's page is built with its toolchain: run nub install in site/");
execFileSync("nub", ["run", "build:play"], { cwd: site, env: { ...process.env, REYLO_PLAY_OUT: join(reylo, "play") }, stdio: ["ignore", "ignore", "inherit"] });
const page = readdirSync(join(reylo, "play"), { recursive: true, encoding: "utf8" });
if (!page.includes("index.html")) throw new Error("dist/reylo/play has no index.html");
console.log(`dist/reylo/play: ${page.length} files, ${Math.round(size(join(reylo, "play")) / 1024)} KB`);

// The engines: every platform's from the binaries release.yml built on each (its artifacts, one
// directory per platform), or this machine's alone.
const host = hostPlatform();
const binaries = new Map<Platform, string>();
if (args.binaries) {
  for (const p of PLATFORMS) {
    const path = join(args.binaries, `engine-${p.platform}`, p.os === "win32" ? "reylo.exe" : "reylo");
    if (!existsSync(path)) throw new Error(`${path} is missing: --binaries needs every platform's engine (npm/platforms.json)`);
    binaries.set(p, path);
  }
} else {
  if (!host) throw new Error(`no platform package for ${process.platform}-${process.arch}: npm/platforms.json names the platforms reylo ships`);
  let path = args.binary;
  if (!path) {
    execFileSync("cargo", ["build", "--release", "--locked", "-p", "reylo"], { cwd: root, env: { ...process.env, ...RELEASE_ENV }, stdio: "inherit" });
    path = join(root, "target", "release", host.os === "win32" ? "reylo.exe" : "reylo");
  }
  binaries.set(host, path);
}

for (const [p, from] of binaries) {
  const name = `@reylosql/engine-${p.platform}`;
  const to = join(dist, `engine-${p.platform}`);
  const file = join(to, "bin", p.os === "win32" ? "reylo.exe" : "reylo");
  mkdirSync(join(to, "bin"), { recursive: true });
  copyFileSync(from, file);
  // A binary that travelled as a CI artifact has lost its mode; npm packs the one it finds here.
  chmodSync(file, 0o755);
  const found = executableFormat(readFileSync(file));
  const wanted = { os: p.os, cpu: p.cpu, libc: p.libc };
  if (!found || found.os !== wanted.os || found.cpu !== wanted.cpu || found.libc !== wanted.libc) {
    throw new Error(`${from} is ${found ? describe(found) : "no executable reylo knows"}, and ${name} is ${describe(wanted)}`);
  }
  if (p === host) {
    const printed = execFileSync(file, ["--version"], { encoding: "utf8" }).trim();
    if (printed !== `reylo ${pkg.version}`) throw new Error(`${from} prints "${printed}", and reylo is ${pkg.version}; rebuild it`);
  }
  const engine = {
    name,
    version: pkg.version,
    description: `The reylo binary for ${p.name}: Reylo's analysis engine. Install reylo, which depends on it.`,
    license: pkg.license,
    author: pkg.author,
    homepage: pkg.homepage,
    repository: { ...pkg.repository, directory: "npm/engine" },
    bugs: pkg.bugs,
    os: [p.os],
    cpu: [p.cpu],
    ...(p.libc ? { libc: [p.libc] } : {}),
    // Yarn's Plug'n'Play would otherwise keep the binary inside a zip archive, where nothing can start it.
    preferUnplugged: true,
    files: ["bin"],
  };
  writeFileSync(join(to, "package.json"), JSON.stringify(engine, null, 2) + "\n");
  writeFileSync(join(to, "README.md"), readFileSync(join(root, "npm", "engine", "README.md"), "utf8").replaceAll("{platform}", p.platform).replaceAll("{name}", p.name));
  copyFileSync(join(root, "LICENSE"), join(to, "LICENSE"));
  console.log(`dist/engine-${p.platform}: package ${name}@${pkg.version}, bin/${relative(join(to, "bin"), file)} ${statSync(file).size} bytes`);
}

// A project that links dist/reylo, as the examples do, finds this machine's engine where an install
// puts it: npm packs no node_modules, so the link ships nowhere.
if (host && binaries.has(host)) {
  mkdirSync(join(reylo, "node_modules", "@reylosql"), { recursive: true });
  // A junction on Windows, which needs no privilege and takes an absolute target.
  const target = process.platform === "win32" ? join(dist, `engine-${host.platform}`) : join("..", "..", "..", `engine-${host.platform}`);
  symlinkSync(target, join(reylo, "node_modules", "@reylosql", `engine-${host.platform}`), "junction");
}

function size(path: string): number {
  return statSync(path).isDirectory() ? readdirSync(path).reduce((n, f) => n + size(join(path, f)), 0) : statSync(path).size;
}

/** The platform package this machine installs, as a package manager picks it: by `os`, `cpu` and, on Linux, the C library. */
function hostPlatform(): Platform | undefined {
  let libc: string | undefined;
  if (process.platform === "linux") {
    // Node's report names the glibc it runs on, and names none on musl.
    const header = (process.report.getReport() as { header: { glibcVersionRuntime?: string } }).header;
    libc = header.glibcVersionRuntime ? "glibc" : "musl";
  }
  return PLATFORMS.find((p) => p.os === process.platform && p.cpu === process.arch && p.libc === libc);
}

type Format = { os: string; cpu: string; libc?: string | undefined };

function describe(f: Format): string {
  return `${f.os}-${f.cpu}${f.libc ? ` (${f.libc})` : ""}`;
}

/** What a file's header says it runs on, or undefined for a file that is no executable: a Mach-O, an ELF or a PE file, its CPU, and for ELF the C library its program interpreter names. */
function executableFormat(b: Buffer): Format | undefined {
  if (b.length < 64) return undefined;
  if (b.readUInt32LE(0) === 0xfeedfacf) {
    const cpu = { 0x0100000c: "arm64", 0x01000007: "x64" }[b.readUInt32LE(4)];
    return cpu ? { os: "darwin", cpu } : { os: "darwin", cpu: "unknown" };
  }
  if (b.readUInt32BE(0) === 0x7f454c46) {
    const cpu = { 0x3e: "x64", 0xb7: "arm64" }[b.readUInt16LE(18)] ?? "unknown";
    // A glibc program names glibc's loader; a musl one is static, with no interpreter, or names musl's.
    let interpreter = "";
    const phoff = Number(b.readBigUInt64LE(32));
    const entry = b.readUInt16LE(54);
    for (let i = 0; i < b.readUInt16LE(56); i++) {
      const at = phoff + i * entry;
      if (at + entry > b.length) break;
      if (b.readUInt32LE(at) === 3) {
        const offset = Number(b.readBigUInt64LE(at + 8));
        interpreter = b.subarray(offset, offset + Number(b.readBigUInt64LE(at + 32))).toString("latin1");
      }
    }
    return { os: "linux", cpu, libc: interpreter.includes("ld-linux") ? "glibc" : "musl" };
  }
  if (b.readUInt16LE(0) === 0x5a4d) {
    const pe = b.readUInt32LE(0x3c);
    if (pe + 6 > b.length || b.readUInt32LE(pe) !== 0x00004550) return undefined;
    const cpu = { 0x8664: "x64", 0xaa64: "arm64" }[b.readUInt16LE(pe + 4)] ?? "unknown";
    return { os: "win32", cpu };
  }
  return undefined;
}
