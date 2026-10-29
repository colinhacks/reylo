// Builds every publishable package into dist/, one directory each, published with
// `npm publish dist/<name>`:
// - dist/reylo: the run time and the entry points a project names (tsconfig.build.json), compiled to
//   JavaScript with their declarations, the README, `reylo play`'s page in play/ (built from site/),
//   and a package.json whose exports and content mapper name the compiled files. Node strips no types under node_modules, so an installed copy
//   that ships TypeScript fails to load in the mapper, the bundler adapters and at run time. It
//   carries no engine and no build step: each database's is a package of its own, an optional peer
//   at reylo's exact version, which the entry points find from the project (src/engine-package.ts).
// - dist/postgres, dist/sqlite, dist/mysql, dist/duckdb: `@reylosql/<database>`, at reylo's version, with its
//   engine component in `engine/` (built by engine/build.sh and committed), its package.json and
//   README, reylo's build-time half (src/tooling.ts) bundled into `tooling.js`, the worker the
//   engine opens its connections in (src/engine-worker.ts) bundled into `engine-worker.js`, and the
//   I/O worker of jco's preview2-shim, which both bundle, beside them. Nothing in it is
//   platform-specific, and it depends on nothing.
// The repository's own package.json files keep pointing at the sources, so the tests and the labs
// run them without a build; every one of them is private, so that none can be published by
// mistake.
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { build as bundle, transformSync } from "esbuild";

const root = join(import.meta.dirname, "..");
const dist = join(root, "dist");
const js = (path: string) => path.replace(/\.ts$/, ".js");
const DATABASES = ["postgres", "sqlite", "mysql", "duckdb"] as const;

rmSync(dist, { recursive: true, force: true });
const reylo = join(dist, "reylo");
execFileSync(join(root, "node_modules/.bin/tsc"), ["-p", join(root, "tsconfig.build.json")], { stdio: "inherit" });
// The comments stay in the sources and in the declarations, where an editor shows them. The
// JavaScript ships without them, since they are about a third of its size.
for (const file of readdirSync(reylo, { recursive: true, encoding: "utf8" }).filter((f) => f.endsWith(".js"))) {
  const path = join(reylo, file);
  writeFileSync(path, transformSync(readFileSync(path, "utf8"), { loader: "js", legalComments: "inline" }).code);
}
copyFileSync(join(root, "README.md"), join(reylo, "README.md"));

const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const { private: _private, scripts: _scripts, devDependencies: _dev, bin, exports, typescript, ...rest } = pkg;
// The engines are peers at reylo's own version: reylo refuses another (src/engine.ts).
for (const db of DATABASES) {
  const name = `@reylosql/${db}`;
  if (rest.peerDependencies?.[name] !== pkg.version) throw new Error(`package.json: the peer ${name} is ${rest.peerDependencies?.[name]}, and reylo is ${pkg.version}; they move together`);
  if (!rest.peerDependenciesMeta?.[name]?.optional) throw new Error(`package.json: the peer ${name} must be optional, since a project installs only its databases' engines`);
}
const built = {
  ...rest,
  // A bin path without its "./", as npm writes it: npm rewrites `./src/cli.js` to `src/cli.js` at
  // publish and warns that the script "was invalid and removed", which it was not (0.0.1's stage).
  bin: Object.fromEntries(Object.entries(bin as Record<string, string>).map(([name, path]) => [name, js(path).replace(/^\.\//, "")])),
  exports: Object.fromEntries(Object.entries(exports as Record<string, string>).map(([key, path]) => [key, { types: path.replace(/\.ts$/, ".d.ts"), default: js(path) }])),
  typescript: { ...typescript, contentMapper: { ...typescript.contentMapper, exec: (typescript.contentMapper.exec as string[]).map(js) } },
};
writeFileSync(join(reylo, "package.json"), JSON.stringify(built, null, 2) + "\n");

// Nothing TypeScript ships but declarations, and no engine ships inside reylo.
const files = readdirSync(reylo, { recursive: true, encoding: "utf8" });
const source = files.filter((f) => /\.(c|m)?ts$/.test(f) && !/\.d\.(c|m)?ts$/.test(f));
if (source.length) throw new Error(`dist/reylo holds TypeScript sources: ${source.join(", ")}`);
const wasm = files.filter((f) => f.endsWith(".wasm"));
if (wasm.length) throw new Error(`dist/reylo holds an engine: ${wasm.join(", ")}`);
// Nor any of the build-time half, which a run-time module would drag in by importing it.
const tooling = files.filter((f) => /^src\/(tooling|analyze|engine|engine-core|engine-worker|component|loader|commands|mapper-server|play)\.js$/.test(f));
if (tooling.length) throw new Error(`dist/reylo holds the engine packages' build steps: ${tooling.join(", ")}`);
console.log(`dist/reylo: ${files.length} files, package ${built.name}@${built.version}`);

// `reylo play`'s page (src/play.ts), built from the docs site's playground with the site's own
// toolchain into dist/reylo/play/, which needs the site's install (\`nub install\` in site/).
const site = join(root, "site");
if (!existsSync(join(site, "node_modules", "vite"))) throw new Error("site/ is not installed, and reylo play's page is built with its toolchain: run nub install in site/");
execFileSync("nub", ["run", "build:play"], { cwd: site, env: { ...process.env, REYLO_PLAY_OUT: join(reylo, "play") }, stdio: ["ignore", "ignore", "inherit"] });
const page = readdirSync(join(reylo, "play"), { recursive: true, encoding: "utf8" });
if (!page.includes("index.html")) throw new Error("dist/reylo/play has no index.html");
const pageBytes = page.reduce((n, f) => n + (statSync(join(reylo, "play", f)).isFile() ? statSync(join(reylo, "play", f)).size : 0), 0);
console.log(`dist/reylo/play: ${page.length} files, ${Math.round(pageBytes / 1024)} KB`);

// Each engine package's JavaScript is bundled into a few files with everything it reads, which also
// leaves out the comments.
const bundled = { bundle: true, platform: "node", format: "esm", target: "node24", legalComments: "inline", logLevel: "warning", metafile: true } as const;
// jco's preview2-shim, which tooling.js and engine-worker.js bundle, starts its own I/O worker from
// this file beside the module that bundles it, and loads its native addon (jco-node-fs) only for a
// call the engine never makes, through a `require` of its own that the bundle leaves out.
const shimWorker = join(dirname(createRequire(join(root, "package.json")).resolve("@bytecodealliance/preview2-shim/instantiation")), "..", "io", "worker-thread.bundle.js");

for (const db of DATABASES) {
  const from = join(root, "packages", db);
  const to = join(dist, db);
  mkdirSync(to);
  const { private: _p, exports: entries, devDependencies: _dev, ...engine } = JSON.parse(readFileSync(join(from, "package.json"), "utf8"));
  if (engine.version !== pkg.version) throw new Error(`packages/${db}/package.json is ${engine.version}, and reylo is ${pkg.version}; they move together`);
  const exported = Object.fromEntries(Object.entries(entries as Record<string, string>).map(([key, path]) => [key, js(path)]));
  writeFileSync(join(to, "package.json"), JSON.stringify({ ...engine, exports: exported }, null, 2) + "\n");
  cpSync(join(from, "engine"), join(to, "engine"), { recursive: true });
  copyFileSync(join(from, "README.md"), join(to, "README.md"));
  // Before anything loads the bundles: loading the shim starts its I/O worker from this file.
  copyFileSync(shimWorker, join(to, "worker-thread.bundle.js"));
  const half = await bundle({ ...bundled, entryPoints: [join(root, "src/tooling.ts")], outfile: join(to, "tooling.js") });
  // reylo's entry points load it with a `require` (src/engine-package.ts), which refuses an ES
  // module with a top-level await.
  if (typeof (createRequire(import.meta.url)(join(to, "tooling.js")) as { default?: { sqlModule?: unknown } }).default?.sqlModule !== "function") throw new Error(`dist/${db}/tooling.js does not load with a require`);
  // The engine connects to its database itself, or DuckDB's through the app's own driver: no
  // driver, and nothing native, is bundled.
  const native = Object.keys(half.metafile.inputs).filter((input) => /node_modules\/(postgres|mysql2|@duckdb|@bytecodealliance\/jco-node-fs)\//.test(input));
  if (native.length) throw new Error(`dist/${db}/tooling.js bundles a driver or a native addon: ${native.join(", ")}`);
  await bundle({ ...bundled, entryPoints: [join(root, "src/engine-worker.ts")], outfile: join(to, "engine-worker.js") });
  // A child process src/sqlite-drivers.ts starts beside the module that starts it.
  await bundle({ ...bundled, entryPoints: [join(root, "src/d1-proxy.ts")], outfile: join(to, "d1-proxy.js") });
  const shipped = readdirSync(to);
  const missing = (engine.files as string[]).filter((f) => !shipped.includes(f));
  if (missing.length) throw new Error(`dist/${db} is missing what its package.json lists: ${missing.join(", ")}`);
  const size = (path: string): number => (statSync(path).isDirectory() ? readdirSync(path).reduce((n, f) => n + size(join(path, f)), 0) : statSync(path).size);
  const sizes = shipped.filter((f) => f !== "package.json" && f !== "README.md").map((f) => `${f} ${size(join(to, f))} bytes`);
  console.log(`dist/${db}: package ${engine.name}@${engine.version}, ${sizes.join(", ")}`);
}
