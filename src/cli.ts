#!/usr/bin/env node
// `reylo generate`: analyzes every `.sql` file of the project into `.reylo/`, the cache that the type
// check and the build steps share, and prints each file with a mark and every error in full. A
// build that runs where the database cannot be reached (a CI job, a container build) runs on what
// it wrote; where the database can be reached, it only saves the build the round trips.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { styleText } from "node:util";
import { glob } from "tinyglobby";
import { Session } from "./analyze.ts";
import { findProjectFile, PROJECT_FILE } from "./project.ts";

const USAGE = `Usage: reylo generate [paths...] [--force]

Analyzes every .sql file under the paths (by default the current directory) into .reylo/: the
cache that the type check and the build steps read. A file that is not one query (a migration)
is skipped, and a run that finds no query fails. --force analyzes each file against the database
even when .reylo/ holds it.

Which files are queries can be set in .reylo.yaml; the globs are relative to it:

  cli:
    include: ["src/**/*.sql"]   # instead of every .sql file
    exclude: ["src/legacy/**"]  # in addition to dependencies, build output and what git ignores`;

// Never the project's own queries: dependencies and build output. A directory whose name starts
// with a dot (.git, .reylo, .next) is left out too, because a glob's `*` does not match a dot, but
// for `.server`: React Router's (and Remix's) server-only modules, which its build keeps out of the
// browser, and so the place for an app's queries there (examples/vite).
const INCLUDE = ["**/*.sql", "**/.server/**/*.sql"];
const EXCLUDE = ["**/node_modules/**", "**/{dist,build,out,target,coverage}/**"];

const shown = (path: string) => relative(process.cwd(), path) || ".";
const paint = (format: Parameters<typeof styleText>[0], text: string) => styleText(format, text);

async function sqlFiles(dir: string, include = INCLUDE, exclude: string[] = []): Promise<string[]> {
  const files = await glob(include, { cwd: dir, ignore: [...EXCLUDE, ...exclude], absolute: true });
  return files.filter((f) => f.endsWith(".sql"));
}

/** The files among `files` that git ignores; none where git is missing or `dir` is not in a repository. */
function gitIgnored(dir: string, files: string[]): Set<string> {
  if (!files.length) return new Set();
  const r = spawnSync("git", ["check-ignore", "-z", "--stdin"], { cwd: dir, input: files.join("\0"), encoding: "utf8" });
  // 0: some are ignored; 1: none is; 128 or no status: not a repository, or no git.
  return r.status === 0 ? new Set(r.stdout.split("\0").filter(Boolean).map((f) => resolve(dir, f))) : new Set();
}

/** The place of a byte offset and the line that holds it, with a caret under the span, as an editor underlines it. */
function located(text: string, start: number, length: number): { at: string; frame: string[] } {
  const bytes = Buffer.from(text, "utf8");
  const before = bytes.subarray(0, start).toString("utf8");
  const lineNumber = before.split("\n").length;
  const column = before.length - before.lastIndexOf("\n") - 1;
  const line = text.split("\n")[lineNumber - 1]!.replace(/\r$/, "");
  const span = Math.max(1, Math.min(bytes.subarray(start, start + length).toString("utf8").length, line.length - column));
  const gutter = String(lineNumber);
  return {
    at: `${lineNumber}:${column + 1}`,
    frame: [`  ${paint("dim", `${gutter} |`)} ${line}`, `  ${paint("dim", `${" ".repeat(gutter.length)} |`)} ${line.slice(0, column).replace(/[^\t]/g, " ")}${paint("red", "^".repeat(span))}`],
  };
}

// A file that holds DDL or several statements is not a query: a migration or a schema file, which
// nothing imports. The engine says so (or the server refuses to prepare several commands).
const NOT_A_QUERY = /^A typed \.sql file holds one .* and this one holds (several|another kind), |^cannot insert multiple commands into a prepared statement/;

async function generate(args: string[]): Promise<number> {
  const force = args.includes("--force");
  const unknown = args.filter((a) => a.startsWith("-") && a !== "--force");
  if (unknown.length) {
    console.error(`reylo generate: unknown option ${unknown.join(", ")}\n\n${USAGE}`);
    return 2;
  }
  const paths = args.filter((a) => !a.startsWith("-"));
  const started = performance.now();

  // A file named on the command line is analyzed as named; a directory is searched.
  const named = new Set<string>();
  const found = new Set<string>();
  for (const arg of paths.length ? paths : ["."]) {
    const full = resolve(arg);
    if (!existsSync(full)) {
      console.error(`reylo generate: ${arg} does not exist.`);
      return 2;
    }
    if (statSync(full).isDirectory()) {
      const files = await sqlFiles(full);
      const ignored = gitIgnored(full, files);
      for (const file of files) if (!ignored.has(file)) found.add(file);
    } else if (full.endsWith(".sql")) named.add(full);
    else {
      console.error(`reylo generate: ${arg} is not a .sql file or a directory.`);
      return 2;
    }
  }

  // One project per `.reylo.yaml` (or package): each has its database, its `.reylo/` and its `cli` globs.
  const projects = new Map<Session, string[]>();
  for (const file of [...named, ...found].sort()) {
    const session = await Session.for(dirname(file));
    projects.set(session, [...(projects.get(session) ?? []), file]);
  }
  for (const [session, list] of projects) {
    const { include, exclude } = session.project.cli;
    if (!include && !exclude) continue;
    const allowed = new Set(await sqlFiles(session.root, include, exclude));
    projects.set(session, list.filter((f) => named.has(f) || allowed.has(f)));
  }

  let typed = 0;
  const failed: string[] = [];
  const skipped: string[] = [];
  const errors: string[][] = [];
  for (const [session, list] of projects) {
    if (!list.length) continue;
    const { project } = session;
    const here = session.root === process.cwd() ? "" : `${shown(session.root)}: `;
    console.log(paint("dim", `${here}${project.dialect} at ${session.where()} (${project.urlSource})`));
    // A project that cannot be used at all is said once, above its files.
    let unusable = project.error ?? (project.url ? undefined : project.missingUrl);
    // A file that sets `url` alone is told once, under the line that names the connection used
    // instead; where there is none, the error says it.
    if (project.notice && project.url) console.log(paint("yellow", project.notice));
    let offline = false;
    if (!unusable) {
      const schema = await session.schema();
      if (schema.fatal) unusable = schema.error;
      else if (schema.value === null) {
        offline = true;
        console.log(paint("dim", `${session.cannotConnect(schema.error)}; the files are read from .reylo/ as they are.`));
      }
    }
    if (unusable) errors.push([paint("red", unusable)]);
    const rows: { file: string; ok: boolean }[] = [];
    for (const file of list) {
      const fail = (...lines: string[]) => {
        rows.push({ file, ok: false });
        errors.push(lines);
      };
      if (unusable) {
        rows.push({ file, ok: false });
        continue;
      }
      const sql = readFileSync(file, "utf8");
      const outcome = await session.analyze(sql, { force });
      if (!outcome.ok) {
        fail(`${paint("bold", shown(file))}: ${offline ? (force ? "--force needs the database, which cannot be reached" : "not in .reylo/, and the database cannot be reached") : outcome.message}`);
        continue;
      }
      const { diagnostics } = outcome.analysis;
      if (diagnostics.some((d) => NOT_A_QUERY.test(d.message))) {
        skipped.push(file);
        continue;
      }
      if (diagnostics.length) {
        fail(...diagnostics.flatMap((d) => {
          const { at, frame } = located(sql, d.start, d.length);
          // An error that spans the whole statement has no place in it (SQLite names none).
          const whole = d.start === 0 && d.length >= Buffer.byteLength(sql.trimEnd());
          return [`${paint("bold", `${shown(file)}:${at}`)}: ${d.message}`, ...(whole ? [] : frame)];
        }));
        continue;
      }
      rows.push({ file, ok: true });
    }
    const width = Math.max(...rows.map((r) => shown(r.file).length));
    for (const { file, ok } of rows) {
      if (ok) typed++;
      else failed.push(file);
      console.log(`  ${shown(file).padEnd(width)}  ${ok ? paint("green", "✓") : paint("red", "✗")}`);
    }
  }
  for (const session of Session.opened()) await session.close();

  for (const lines of errors) console.log(`\n${lines.join("\n")}`);
  if (skipped.length) {
    const listed = skipped.slice(0, 3).map(shown).join(", ") + (skipped.length > 3 ? ` and ${skipped.length - 3} more` : "");
    console.log(paint("dim", `\nSkipped ${skipped.length === 1 ? "1 file that is" : `${skipped.length} files that are`} not one query (a migration?): ${listed}`));
  }
  const seconds = ((performance.now() - started) / 1000).toFixed(1);
  const total = typed + failed.length;
  if (failed.length) console.log(`\n${paint("red", "✗")} ${failed.length} of ${total} ${total === 1 ? "query" : "queries"} failed (${seconds} s)`);
  else if (typed) console.log(`\n${paint("green", "✓")} ${typed} ${typed === 1 ? "query" : "queries"} typed (${seconds} s)`);
  // Nothing to build from: a CI job that carries .reylo/ to a build would fail there instead.
  else {
    // The hint names the project's own file where the run covered one project that has one; where
    // no .sql file opened a project, the file of the one directory searched.
    const sessions = [...projects.keys()];
    const own = sessions.length === 1 ? sessions[0]!.project.file : !sessions.length && paths.length <= 1 ? findProjectFile(resolve(paths[0] ?? ".")) : undefined;
    console.log(`${skipped.length ? "\n" : ""}No query under ${(paths.length ? paths : ["."]).join(", ")}: name the directory that holds them, or set cli.include in ${own ? shown(own) : PROJECT_FILE}.`);
    return 1;
  }
  return failed.length ? 1 : 0;
}

const [command, ...rest] = process.argv.slice(2);
if (command === "generate") process.exitCode = await generate(rest);
else if (command === undefined || command === "help" || command === "--help" || command === "-h") console.log(USAGE);
else {
  console.error(`reylo: unknown command ${command}\n\n${USAGE}`);
  process.exitCode = 2;
}
