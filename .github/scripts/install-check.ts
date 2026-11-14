// Installs reylo as a project does, from the packages a run built, on the platform it runs on, and
// checks that the engine installed for that platform types and runs a query: the nine packages
// published to a registry of the check's own (Verdaccio, holding nothing else), `npm install reylo`
// in a fresh project, which must bring this platform's engine package alone, `reylo analyze` on a
// SQLite query through the `reylo` command npm installed, a program run under the reylo/register
// preload, and Postgres statements nested to the engine's bound, analyzed by the installed binary
// as tests/depth.test.ts analyzes them.
//
//   node .github/scripts/install-check.ts <packs> <platform>
//
// <packs> holds the packed packages (`npm pack` of each directory scripts/build.ts writes), and
// <platform> is the npm/platforms.json platform this machine is, whose package must be the one
// installed. Plain Node runs it, on every platform and inside the Linux images binaries.yml names.
import { spawn, spawnSync, type SpawnSyncOptions } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { pathToFileURL } from "node:url";

const [packsArg, platform] = process.argv.slice(2);
if (!packsArg || !platform) throw new Error("usage: node .github/scripts/install-check.ts <packs> <platform>");
const packs = resolve(packsArg);
const VERDACCIO = "6.11.1";
const PORT = 48731;
const registry = `http://127.0.0.1:${PORT}/`;
const windows = process.platform === "win32";

// The installed reylo finds its engine by itself, as a project's does.
delete process.env.REYLO_ENGINE_PATH;
const root = mkdtempSync(join(tmpdir(), "reylo-install-"));
const step = (what: string) => console.log(`\n== ${what}`);

/** A command run to its end with its output shown, which must exit 0; npm and a `.cmd` through a shell on Windows, where npm is npm.cmd. */
function run(command: string, args: string[], options: SpawnSyncOptions = {}): string {
  const r = spawnSync(command, args, { encoding: "utf8", shell: windows && (command === "npm" || command.endsWith(".cmd")), ...options });
  const out = `${r.stdout ?? ""}${r.stderr ?? ""}`;
  if (r.status !== 0) throw new Error(`${command} ${args.join(" ")} exited with ${r.status ?? r.signal}${r.error ? `: ${r.error.message}` : ""}\n${out}`);
  return String(r.stdout ?? "");
}

const registryDir = join(root, "registry");
mkdirSync(registryDir);
writeFileSync(join(registryDir, "config.yaml"), [
  "storage: ./storage",
  "auth:",
  "  htpasswd:",
  "    file: ./htpasswd",
  "    max_users: 1",
  "packages:",
  "  '**':",
  "    access: $all",
  "    publish: $authenticated",
  "log: { type: stdout, format: pretty, level: warn }",
  "max_body_size: 100mb",
  "",
].join("\n"));

step(`Verdaccio ${VERDACCIO}, a registry of this check's own`);
run("npm", ["install", "--prefix", registryDir, `verdaccio@${VERDACCIO}`, "--no-audit", "--no-fund", "--registry", "https://registry.npmjs.org/"]);
const server = spawn(process.execPath, [join(registryDir, "node_modules", "verdaccio", "bin", "verdaccio"), "--config", join(registryDir, "config.yaml"), "--listen", `127.0.0.1:${PORT}`], { cwd: registryDir, stdio: "inherit" });

try {
  for (let tries = 0; ; tries++) {
    try {
      if ((await fetch(`${registry}-/ping`)).ok) break;
    } catch {}
    if (tries > 120) throw new Error("Verdaccio did not answer within a minute");
    await new Promise((done) => setTimeout(done, 500));
  }
  // A user of the registry's own, through the endpoint `npm adduser` posts to, and its token.
  const user = await fetch(`${registry}-/user/org.couchdb.user:check`, { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ name: "check", password: "check-password", type: "user" }) });
  const { token } = (await user.json()) as { token?: string };
  if (!token) throw new Error(`Verdaccio made no user: ${user.status}`);
  const npmrc = join(root, ".npmrc");
  writeFileSync(npmrc, `registry=${registry}\n//127.0.0.1:${PORT}/:_authToken=${token}\n`);

  step("The nine packages, published to it");
  const tarballs = readdirSync(packs).filter((f) => f.endsWith(".tgz")).sort();
  if (tarballs.length !== 9) throw new Error(`${packs} holds ${tarballs.length} packages, not reylo and its eight engines: ${tarballs.join(", ")}`);
  // The engines before reylo, as the release stages them.
  for (const tgz of [...tarballs.filter((f) => !f.startsWith("reylo-")), ...tarballs.filter((f) => f.startsWith("reylo-"))]) {
    run("npm", ["publish", join(packs, tgz), "--userconfig", npmrc, "--provenance=false"]);
    console.log(`published ${tgz}`);
  }

  step("npm install reylo, in a fresh project");
  const version = tarballs.find((f) => f.startsWith("reylo-"))!.replace(/^reylo-/, "").replace(/\.tgz$/, "");
  const project = join(root, "project");
  mkdirSync(project);
  writeFileSync(join(project, "package.json"), JSON.stringify({ name: "check", private: true, type: "module", dependencies: { reylo: version } }, null, 2));
  run("npm", ["install", "--userconfig", npmrc, "--no-audit", "--no-fund"], { cwd: project });
  const engines = readdirSync(join(project, "node_modules", "@reylosql"));
  console.log(`installed: ${engines.join(", ")}`);
  if (engines.join() !== `engine-${platform}`) throw new Error(`npm installed ${engines.join(", ") || "no engine"}, where this machine's is engine-${platform} alone`);
  const binary = join(project, "node_modules", "@reylosql", `engine-${platform}`, "bin", windows ? "reylo.exe" : "reylo");
  const printed = run(binary, ["--version"]).trim();
  if (printed !== `reylo ${version}`) throw new Error(`the installed binary prints "${printed}", where reylo is ${version}`);
  console.log(printed);

  step("reylo analyze through the installed command, and the preload, on a SQLite database");
  const db = new DatabaseSync(join(project, "app.db"));
  db.exec("create table users (id integer primary key, name text not null); insert into users values (1, 'ada')");
  db.close();
  writeFileSync(join(project, "user.sql"), "select id, name from users where id = :id");
  writeFileSync(join(project, "main.ts"), `import { DatabaseSync } from "node:sqlite";\nimport user from "./user.sql";\nconsole.log(JSON.stringify(await user.run(new DatabaseSync("app.db"), { id: 1 })));\n`);
  const env = { ...process.env, DATABASE_URL: "file:./app.db" };
  // npm's own shim for reylo's bin, which starts the launcher (src/cli.ts), which runs the binary.
  const command = join(project, "node_modules", ".bin", windows ? "reylo.cmd" : "reylo");
  const analyzed = run(command, ["analyze", "user.sql", "--format", "json"], { cwd: project, env }).trim();
  console.log(analyzed);
  const descriptor = JSON.parse(analyzed) as { file?: string; result?: { columns: { name: string }[]; cardinality: string } };
  if (descriptor.file !== "user.sql" || descriptor.result?.columns.map((c) => c.name).join() !== "id,name" || descriptor.result?.cardinality !== "at_most_one") throw new Error("reylo analyze did not type user.sql");
  // The playground's server, which the launcher runs in JavaScript.
  if (!run(command, ["play", "--help"], { cwd: project }).startsWith("Usage: reylo play")) throw new Error("reylo play --help printed no usage");
  const ran = run(process.execPath, ["--import", "reylo/register", "main.ts"], { cwd: project, env }).trim();
  console.log(ran);
  if (ran !== `{"id":1,"name":"ada"}`) throw new Error(`the preload's run printed ${ran}`);
  if (!existsSync(join(project, "reylo", ".cache"))) throw new Error("the preload's build step wrote no reylo/.cache/");

  step("Postgres statements nested to the engine's bound");
  const { engine } = (await import(pathToFileURL(join(project, "node_modules", "reylo", "src", "engine.js")).href)) as typeof import("../../src/engine.ts");
  const postgres = await engine("postgres");
  // A server that describes every statement as one int4 column and answers every catalog query
  // with no rows, as tests/depth.test.ts's does: the engine parses the statement and walks it whole.
  const io = {
    async describe(sql: string) {
      const params = new Set(sql.match(/\$\d+/g) ?? []).size;
      return { params: Array.from({ length: params }, () => ({ oid: 23 })), columns: [{ name: "x", oid: 23, table: 0, attnum: 0 }] };
    },
    async query() {
      return [];
    },
  };
  const fold = (n: number, start: string, f: (s: string, i: number) => string) => Array.from({ length: n }, (_, i) => i).reduce(f, start);
  const deepest: [string, string][] = [
    ["sum, 496 deep", `select ${Array(496).fill("id").join(" + ")} as x from users`],
    ["case, 493 deep", `select ${fold(493, "0", (s, i) => `case when id = ${i} then ${i} else ${s} end`)} as x from users`],
    ["json_build_object, 495 deep", `select ${fold(495, "id", (s) => `json_build_object('a', ${s})`)} as x from users`],
    ["subquery, 164 deep", `select (${fold(164, "select 1", (s) => `select (${s})`)}) as x from users`],
  ];
  for (const [shape, sql] of deepest) {
    const at = await postgres.analyze(sql, "postgres", io);
    if (at.kind !== "select" || at.cardinalityReason === "the statement did not parse locally") throw new Error(`the engine did not read the ${shape} statement whole`);
    console.log(`${shape}: read whole`);
  }
  const past = await postgres.analyze(`select (${fold(165, "select 1", (s) => `select (${s})`)}) as x from users`, "postgres", io);
  if (past.cardinalityReason !== "the statement did not parse locally") throw new Error("a statement past the bound was not refused as too deep");
  console.log("subquery, 165 deep: past the bound, typed from the describe");

  console.log(`\n${platform}: installed, typed and ran`);
} finally {
  server.kill();
  // Windows keeps the files of a program that runs, and the engine's children run until this
  // process ends: what is left of the directory goes with the runner's own temporary files.
  try {
    rmSync(root, { recursive: true, force: true, maxRetries: 5 });
  } catch {}
}
