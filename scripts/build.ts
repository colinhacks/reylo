// Builds every publishable package into dist/, one directory each, published with
// `npm publish dist/<name>`:
// - dist/reylo: the sources compiled to JavaScript with their declarations, the README, and a
//   package.json whose exports and content mapper name the compiled files. Node strips no types
//   under node_modules, so an installed copy that ships TypeScript fails to load in the mapper, the
//   bundler adapters and at run time. It carries no engine: each database's is a package of its
//   own, an optional peer at reylo's exact version, which `src/engine.ts` finds from the project.
// - dist/postgres, dist/sqlite, dist/mysql: `@reylosql/<database>`, its `engine.wasm` (built by
//   engine/build.sh and committed) and its package.json and README, at reylo's version.
// The repository's own package.json files keep pointing at the sources, so the tests and the labs
// run them without a build; every one of them is private, so that none can be published by
// mistake.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { transformSync } from "esbuild";

const root = join(import.meta.dirname, "..");
const dist = join(root, "dist");
const js = (path: string) => path.replace(/\.ts$/, ".js");
const DATABASES = ["postgres", "sqlite", "mysql"] as const;

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
console.log(`dist/reylo: ${files.length} files, package ${built.name}@${built.version}`);

for (const db of DATABASES) {
  const from = join(root, "packages", db);
  const to = join(dist, db);
  mkdirSync(to);
  const { private: _p, ...engine } = JSON.parse(readFileSync(join(from, "package.json"), "utf8"));
  if (engine.version !== pkg.version) throw new Error(`packages/${db}/package.json is ${engine.version}, and reylo is ${pkg.version}; they move together`);
  writeFileSync(join(to, "package.json"), JSON.stringify(engine, null, 2) + "\n");
  copyFileSync(join(from, "engine.wasm"), join(to, "engine.wasm"));
  copyFileSync(join(from, "README.md"), join(to, "README.md"));
  const size = readFileSync(join(to, "engine.wasm")).length;
  console.log(`dist/${db}: package ${engine.name}@${engine.version}, engine.wasm ${size} bytes`);
}
