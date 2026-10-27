// The type check on Windows (windows.yml), through the packages `nub run build` writes: one project
// installs them from dist/, and for each way a URL can name its database `reylo generate` analyzes
// its query and `tsc` types it. The engine reaches the files a URL names through jco's preview2-shim,
// which maps a guest path such as `/C:/app/dev.db` to the drive or share (src/component.ts), so each
// case names its database differently: relative, absolute, as a file URL, on another drive, on a
// `subst` drive, through a share, through a short (8.3) name, beside a writer holding a WAL open, and
// for Postgres the TLS files a URL or the environment names.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { DatabaseSync } from "node:sqlite";

const repo = resolve(import.meta.dirname, "..", "..");
const work = process.env.RUNNER_TEMP ?? tmpdir();
const project = join(work, "reylo-windows");
const failures: string[] = [];

const posixRelative = (from: string, to: string) => relative(from, to).replace(/\\/g, "/");
rmSync(project, { recursive: true, force: true });
mkdirSync(join(project, "queries"), { recursive: true });
mkdirSync(join(project, "src"));
writeFileSync(join(project, "package.json"), JSON.stringify({
  name: "reylo-windows",
  private: true,
  type: "module",
  dependencies: { reylo: `link:${posixRelative(project, join(repo, "dist", "reylo"))}` },
  devDependencies: {
    "@reylosql/sqlite": `link:${posixRelative(project, join(repo, "dist", "sqlite"))}`,
    "@reylosql/postgres": `link:${posixRelative(project, join(repo, "dist", "postgres"))}`,
    "@reylosql/mysql": `link:${posixRelative(project, join(repo, "dist", "mysql"))}`,
    "@types/node": "^26.6.1",
    typescript: "7.1.0-beta",
  },
}, null, 2));
writeFileSync(join(project, "tsconfig.json"), JSON.stringify({
  compilerOptions: { target: "esnext", module: "nodenext", lib: ["esnext"], types: ["node"], strict: true, noEmit: true, skipLibCheck: true },
  contentMappers: [{ package: "reylo", extensions: [".sql", ".reylo.yaml", ".reylo.yml"] }],
  include: ["src"],
}, null, 2));
writeFileSync(join(project, "queries", "get_user.sql"), "select id, name from users where id = :id");
writeFileSync(join(project, "src", "main.ts"), [
  `import { DatabaseSync } from "node:sqlite";`,
  `import getUser from "../queries/get_user.sql";`,
  `const db = new DatabaseSync(":memory:");`,
  `const user = getUser.runSync(db, { id: 1 });`,
  `const name: string | undefined = user?.name;`,
  `// @ts-expect-error: the parameter is a number`,
  `getUser.runSync(db, { id: "one" });`,
  `// @ts-expect-error: no such column`,
  `user?.nope;`,
  `console.log(name);`,
  ``,
].join("\n"));
run("nub", ["install"], project, true);

function run(command: string, args: string[], cwd: string, shell = false, env: Record<string, string> = {}) {
  const r = spawnSync(command, args, { cwd, encoding: "utf8", shell, env: { ...process.env, ...env } });
  return { status: r.status, out: `${r.stdout ?? ""}${r.stderr ?? ""}${r.error ? String(r.error) : ""}` };
}

function sqliteDb(path: string, wal = false): DatabaseSync {
  mkdirSync(resolve(path, ".."), { recursive: true });
  rmSync(path, { force: true });
  const db = new DatabaseSync(path);
  if (wal) db.exec("pragma journal_mode = wal; pragma wal_autocheckpoint = 0");
  db.exec("create table users (id integer primary key, name text not null)");
  if (!wal) db.close();
  return db;
}

/** One way to name the database: `reylo generate` must type the query, and for SQLite `tsc` must too. */
function check(name: string, url: string, { env = {}, tsc = true, observe = false }: { env?: Record<string, string>; tsc?: boolean; observe?: boolean } = {}) {
  writeFileSync(join(project, ".reylo.yaml"), `url: ${JSON.stringify(url)}\n`);
  rmSync(join(project, ".reylo"), { recursive: true, force: true });
  const generate = run(process.execPath, [join(project, "node_modules", "reylo", "src", "cli.js"), "generate"], project, false, env);
  const typed = generate.status === 0 && generate.out.includes("✓");
  console.log(`\n== ${name}: ${url}\n${generate.out.trim()}`);
  if (!typed && !observe) failures.push(`${name}: reylo generate exited ${generate.status}`);
  if (!tsc) return;
  rmSync(join(project, ".reylo"), { recursive: true, force: true });
  const check = run(process.execPath, [join(project, "node_modules", "typescript", "bin", "tsc"), "--noEmit", "--runExternalCode"], project, false, env);
  console.log(`tsc: ${check.status}${check.out.trim() ? `\n${check.out.trim()}` : ""}`);
  if (check.status !== 0) failures.push(`${name}: tsc exited ${check.status}`);
}

// SQLite, from the project's own drive.
sqliteDb(join(project, "app.db"));
check("relative file:", "file:./app.db");
sqliteDb(join(project, "data", "app.db"));
check("relative sqlite: in a directory", "sqlite:data/app.db");
check("absolute, backslashes", `file:${join(project, "app.db")}`);
check("absolute, forward slashes", `sqlite:${join(project, "app.db").replace(/\\/g, "/")}`);
check("file URL", pathToFileURL(join(project, "app.db")).href);

// Another drive: the system's temporary directory when it is on one, and a `subst` drive always.
const other = join(tmpdir(), "reylo db é");
sqliteDb(join(other, "app.db"));
console.log(`\nproject on ${project.slice(0, 2)}, temporary directory ${tmpdir()}`);
check("another directory, a space and an accent in its name", `file:${join(other, "app.db")}`);
const subst = run("subst", ["R:", other], project, true);
if (subst.status === 0) check("a subst drive", "file:R:\\app.db");
else failures.push(`subst: ${subst.out}`);
const unc = `\\\\localhost\\${other.slice(0, 1)}$${other.slice(2)}\\app.db`;
if (existsSync(unc)) check("a share", `file:${unc}`);
else failures.push(`the share ${unc} cannot be read on this runner`);

// A writer holding the database in WAL mode, its table still in the -wal file.
const writer = sqliteDb(join(project, "wal.db"), true);
check("a WAL database beside its writer", "file:./wal.db");
writer.close();

// Postgres over TCP, with its TLS files named every way libpq reads them.
const pg = process.env.REYLO_WINDOWS_PG;
const certs = process.env.REYLO_WINDOWS_CERTS;
if (pg && certs) {
  const elsewhere = join(tmpdir(), "reylo certs");
  mkdirSync(elsewhere, { recursive: true });
  for (const f of ["root.crt", "client.crt", "client.key"]) copyFileSync(join(certs, f), join(elsewhere, f));
  mkdirSync(join(project, "certs"), { recursive: true });
  for (const f of ["root.crt", "client.crt", "client.key"]) copyFileSync(join(certs, f), join(project, "certs", f));
  const client = (dir: string) => `&sslcert=${encodeURIComponent(join(dir, "client.crt"))}&sslkey=${encodeURIComponent(join(dir, "client.key"))}`;
  check("Postgres, TLS files on another drive", `${pg}?sslmode=verify-full&sslrootcert=${encodeURIComponent(join(elsewhere, "root.crt"))}${client(elsewhere)}`, { tsc: false });
  check("Postgres, relative TLS files", `${pg}?sslmode=verify-ca&sslrootcert=certs/root.crt&sslcert=certs/client.crt&sslkey=certs/client.key`, { tsc: false });
  check("Postgres, PGSSL* in the environment", pg, { tsc: false, env: { PGSSLMODE: "verify-ca", PGSSLROOTCERT: join(elsewhere, "root.crt"), PGSSLCERT: "certs\\client.crt", PGSSLKEY: join(project, "certs", "client.key") } });
  check("Postgres, NODE_EXTRA_CA_CERTS", `${pg}?sslrootcert=system${client(join(project, "certs"))}`, { tsc: false, env: { NODE_EXTRA_CA_CERTS: join(elsewhere, "root.crt") } });
  // A Unix socket's directory and a named pipe: neither is a TCP port.
  check("Postgres, a socket directory", `postgres://postgres@/reylo_windows?host=/tmp`, { tsc: false, observe: true });
  check("MySQL over TCP", "mysql://root@127.0.0.1:53306/reylo_windows", { tsc: false });
  check("MySQL, a named pipe", `mysql://root@localhost/reylo_windows?socketPath=${encodeURIComponent("\\\\.\\pipe\\reylo-mysql")}`, { tsc: false, observe: true });
}

if (subst.status === 0) run("subst", ["R:", "/D"], project, true);
console.log(failures.length ? `\n${failures.length} failed:\n${failures.map((f) => `- ${f}`).join("\n")}` : "\nevery case passed");
process.exit(failures.length ? 1 : 0);
