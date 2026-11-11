// The project file, `.reylo.yaml` (or `.reylo.yml`), and the connection it names. The file sits
// beside the project's `package.json` (the nearest one at or above the tsconfig's directory;
// `REYLO_CONFIG` names another file), is parsed by the engine so that every reylo tool reads the same YAML, and is
// resolved here against the environment: the process environment first, then the `.env` files the
// project file names under `env` (by default `.env.local`, then `.env`, beside it), and a `${VAR}`
// in a URL is replaced from that. A `$(command)` in `direct_url` is replaced by what the command
// prints, so that a secret can come from a password manager or a cloud's token command instead of
// a file. The URL's scheme names the dialect. The connection is
// `direct_url`, else `${DATABASE_URL}`, and never `url`, which the file keeps for another use
// (wiki/project-config.md section 4). A file that sets `url` without `direct_url` is told so by
// `reylo analyze` and `reylo play`, once, and by the error a project with no connection gets; never by an error of
// its own, since TypeScript makes every content mapper diagnostic one. With no URL, the dialect
// is the one the entries of the project's cache (`reylo/.cache/`) record.
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { type Engine, MissingEngine } from "./engine.ts";
import type { Codecs } from "./codecs.ts";

export type Dialect = "postgres" | "sqlite" | "mysql" | "duckdb";

/** What the engine reads out of `.reylo.yaml`: the keys the library looks at. */
export type ProjectFile = {
  /** Never the type check's connection: read only to say so. */
  url?: string;
  direct_url?: string;
  env?: string[];
  allow_untyped_params?: boolean;
  allow_nested_arrays?: boolean;
  sqlcommenter?: boolean;
  /** The directory of the project's own files, `reylo` beside the project file when absent. */
  dir?: string;
  js?: { codecs?: Codecs };
};

export type ResolvedProject = {
  /** The project file, when there is one. */
  file: string | undefined;
  /** The directory a relative path in the project resolves against: the package's. */
  root: string;
  /** The cache the type check and the build steps share: `.cache/` in the project's `reylo/` directory. */
  cache: string;
  dialect: Dialect;
  /** Whether a parameter the statement does not type is an error rather than `unknown`: true unless the file says `allow_untyped_params: true`. */
  strictParams: boolean;
  /** Whether a Postgres array column may hold nested arrays, typed at any depth and with nothing refused: false unless the file says `allow_nested_arrays: true`. */
  allowNestedArrays: boolean;
  /** Whether each statement carries a sqlcommenter comment naming its file (src/sqlcommenter.ts): true unless the file says `sqlcommenter: false`. */
  sqlcommenter: boolean;
  /** The type `.run()` gives each family of values where the file's `js.codecs` names one (src/codecs.ts). */
  codecs: Codecs;
  /** The connection the type check uses: `direct_url` when the file names one, else `DATABASE_URL`; never `url`. */
  url: string | undefined;
  /** Where the URL came from, for a diagnostic: never the URL itself. */
  urlSource: string;
  /**
   * Where `direct_url` runs a `$(command)`: runs each command again, past the output this process
   * keeps, and returns the URL they make now, or undefined when one fails. The session calls it
   * when the database refuses the login, as when a short-lived token has expired. Set too when a
   * command failed, whose failure is then `error`.
   */
  refreshUrl: (() => string | undefined) | undefined;
  /** The files a change to which changes the project: the project file and the `.env` files. */
  watched: string[];
  /** Why the project cannot be used, already placed (`.reylo.yaml:3:1: ...`); every transform reports it. */
  error: string | undefined;
  /** That the file sets `url`, which Reylo does not read, and no `direct_url`: said once per project, never an error. */
  notice: string | undefined;
  /** What the project is told when it has no connection: where to set one. */
  missingUrl: string;
};

export const PROJECT_FILE = ".reylo.yaml";
/** The names the project file may take; the first is the one messages name. */
export const PROJECT_FILES = [PROJECT_FILE, ".reylo.yml"] as const;
const DEFAULT_ENV_FILES = [".env.local", ".env"];

/** The package directory: the nearest one at or above `from`, below the home directory (`HOME`, or `USERPROFILE` on Windows, in `env`), that holds a `package.json`, else `from`. A home directory is no project's, though a stray `package.json` can sit there, and taking it would make every directory below it one project (engine/cli/src/project.rs). */
export function packageDir(from: string, env: Record<string, string | undefined> = process.env): string {
  const home = process.platform === "win32" ? env.USERPROFILE : env.HOME;
  // As written, or with its links resolved, as a working directory has them.
  const stops = home ? [resolve(home), realOr(home)] : [];
  for (let dir = resolve(from); !stops.includes(dir); dir = dirname(dir)) {
    if (existsSync(join(dir, "package.json"))) return dir;
    if (dirname(dir) === dir) break;
  }
  return resolve(from);
}

function realOr(path: string): string {
  try {
    return realpathSync(path);
  } catch {
    return resolve(path);
  }
}

/**
 * A `.sql` file's path from the root of its own project (the project file's directory, else its
 * package's), written with `/`: the query's `source`, and the file its sqlcommenter comment names.
 */
export function sourceName(path: string, env: Record<string, string | undefined> = process.env): string {
  const dir = dirname(path);
  const file = findProjectFile(dir, env);
  return relative(file ? dirname(file) : packageDir(dir, env), path).split(sep).join("/");
}

/** The project file for a project directory: `REYLO_CONFIG`, or `.reylo.yaml` (else `.reylo.yml`) beside the package's `package.json`. */
export function findProjectFile(from: string, env: Record<string, string | undefined> = process.env): string | undefined {
  if (env.REYLO_CONFIG) return resolve(env.REYLO_CONFIG);
  const dir = packageDir(from, env);
  return PROJECT_FILES.map((name) => join(dir, name)).find((path) => existsSync(path));
}

/** How many of the cache's entries are read for the dialect where no URL names one. */
const CACHED_READ = 32;

/**
 * The dialects the entries in the project's cache record (src/analyze.ts), which name the
 * database where no URL does: a build with no URL reads them. A few entries are read, which a
 * project's are all of one dialect unless its database changed.
 */
function cachedDialects(cache: string): Dialect[] {
  const dir = join(cache, "queries");
  let names: string[];
  try {
    names = readdirSync(dir).filter((n) => n.endsWith(".json"));
  } catch {
    return [];
  }
  const found = new Set<Dialect>();
  for (const name of names.slice(0, CACHED_READ)) {
    try {
      const { dialect } = JSON.parse(readFileSync(join(dir, name), "utf8")) as { dialect?: unknown };
      if (dialect === "postgres" || dialect === "mysql" || dialect === "sqlite" || dialect === "duckdb") found.add(dialect);
    } catch {}
    if (found.size > 1) break;
  }
  return [...found];
}

/** The dialect a URL's scheme names, or undefined for a scheme reylo does not read. */
export function dialectOf(url: string): Dialect | undefined {
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(url)?.[1]?.toLowerCase();
  if (scheme === "postgres" || scheme === "postgresql") return "postgres";
  if (scheme === "mysql" || scheme === "mariadb") return "mysql";
  if (scheme === "file" || scheme === "sqlite") return "sqlite";
  if (scheme === "duckdb") return "duckdb";
  return undefined;
}

/**
 * An env file's variables, none when it is missing or cannot be read, read as dotenv reads them, so
 * that Reylo reads the value the app does. A line ends at `\n`, `\r\n` or `\r`, and is `NAME=value`,
 * `export ` before it or not; a line whose first character is `#`, or that has no `=`, sets nothing.
 * A value in double, single or back quotes is what lies between them, lines included, when nothing
 * but blanks and a comment follows the closing quote on its line; any other value is its line up to
 * the first `#`, spaces and tabs trimmed, less a quote at each end. In a value that starts with a
 * double quote, `\n` and `\r` are a newline and a carriage return. Node's own `parseEnv` is not used:
 * a line whose empty value ends in a space takes the next line as its value there. The binary reads
 * env files the same way (`parse_env` in engine/cli/src/project.rs), and both are held to
 * engine/cli/src/env-file-cases.json.
 */
export function readEnvFile(path: string): Record<string, string> {
  let text: string;
  try {
    text = readFileSync(path, "utf8").replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n");
  } catch {
    return {};
  }
  const lineEnd = (from: number) => {
    const n = text.indexOf("\n", from);
    return n < 0 ? text.length : n;
  };
  const vars: Record<string, string> = Object.create(null);
  for (let at = 0; at < text.length; ) {
    let end = lineEnd(at);
    const line = text.slice(at, end);
    const eq = /^[ \t]*#/.test(line) ? -1 : line.indexOf("=");
    if (eq < 0) {
      at = end + 1;
      continue;
    }
    const name = /^[ \t]*(?:export[ \t]+)?([A-Za-z_][A-Za-z0-9_]*)[ \t]*$/.exec(line.slice(0, eq))?.[1];
    const start = at + eq + 1 + /^[ \t]*/.exec(line.slice(eq + 1))![0].length;
    let raw = text.slice(start, end).split("#")[0]!.replace(/[ \t]+$/, "");
    const quote = text[start];
    if (quote === '"' || quote === "'" || quote === "`") {
      // The quote closes, as in dotenv, at the first quote of its kind that no backslash escapes, or
      // else at an escaped one before it, the last first: at the first of these that nothing but
      // blanks and a comment follows on its line.
      const escaped: number[] = [];
      let close = text.indexOf(quote, start + 1);
      for (; close >= 0 && text[close - 1] === "\\"; close = text.indexOf(quote, close + 1)) escaped.push(close);
      const closes = [...(close >= 0 ? [close] : []), ...escaped.reverse()].find((p) => /^[ \t]*(?:#.*)?$/.test(text.slice(p + 1, lineEnd(p))));
      if (closes !== undefined) {
        raw = text.slice(start, closes + 1);
        end = lineEnd(closes);
      }
    }
    let value = raw.length > 1 && /^["'`]/.test(raw) && raw.endsWith(raw[0]!) ? raw.slice(1, -1) : raw;
    if (raw.startsWith('"')) value = value.replaceAll("\\n", "\n").replaceAll("\\r", "\r");
    if (name) vars[name] = value;
    at = end + 1;
  }
  return vars;
}

/** 1-based line and column of a byte offset, for a message that names the place in the file. */
function place(text: string, offset: number): string {
  const before = Buffer.from(text, "utf8").subarray(0, offset).toString("utf8");
  const lines = before.split("\n");
  return `${lines.length}:${lines.at(-1)!.length + 1}`;
}

/**
 * Where the project file sets `types`, which was removed on 2026-10-20: Reylo gives every value
 * its type itself, one representation per SQL type on every driver (src/values.ts), so nothing
 * is left to set. The engine still reads the key, so the loader refuses it first.
 */
const TYPES_KEY = /^types[ \t]*:/m;

/** How long a `$(command)` in `direct_url` may run: long enough to answer a password manager's prompt. */
const COMMAND_TIMEOUT_MS = 120_000;
/** How long a command's failure stands before the command is run again. */
const FAILED_COMMAND_TTL = 60_000;

/** A part of `direct_url`: text as written, or a `$(command)` whose output takes its place. */
type UrlPart = { text: string } | { command: string };

/**
 * `direct_url` cut into its text and its `$(command)`s. A command ends at the `)` that balances
 * its `$(`, outside quotes, as in a shell. A `$(` left open is the offset it opens at.
 */
/**
 * `text` with each `${VAR}` replaced by `required(VAR)`, and each `${VAR:-fallback}` by `set(VAR)`,
 * or where that is unset or empty by its fallback, read the same way, as a shell reads one. A
 * `${` that opens neither stays as written.
 */
function interpolate(text: string, set: (name: string) => string | undefined, required: (name: string) => string): string {
  let out = "";
  let rest = text;
  for (;;) {
    const at = rest.indexOf("${");
    if (at < 0) return out + rest;
    out += rest.slice(0, at);
    const after = rest.slice(at + 2);
    const opened = /^([A-Za-z_][A-Za-z0-9_]*)(\}|:-)/.exec(after);
    if (!opened) {
      out += "${";
      rest = after;
    } else if (opened[2] === "}") {
      out += required(opened[1]!);
      rest = after.slice(opened[0].length);
    } else {
      const body = after.slice(opened[0].length);
      let depth = 1;
      let end = 0;
      for (; end < body.length; end++) {
        if (body.startsWith("${", end)) {
          depth++;
          end++;
        } else if (body[end] === "}" && --depth === 0) break;
      }
      if (depth !== 0) {
        out += "${";
        rest = after;
      } else {
        out += set(opened[1]!) || interpolate(body.slice(0, end), set, required);
        rest = body.slice(end + 1);
      }
    }
  }
}

function urlParts(written: string): UrlPart[] | { open: number } {
  const parts: UrlPart[] = [];
  let text = "";
  for (let i = 0; i < written.length; ) {
    if (written[i] !== "$" || written[i + 1] !== "(") {
      text += written[i++];
      continue;
    }
    let depth = 1;
    let quote: string | undefined;
    let j = i + 2;
    for (; j < written.length && depth > 0; j++) {
      const c = written[j];
      if (quote) {
        if (c === quote) quote = undefined;
        else if (c === "\\" && quote === '"') j++;
      } else if (c === "'" || c === '"') quote = c;
      else if (c === "\\") j++;
      else if (c === "(") depth++;
      else if (c === ")") depth--;
    }
    if (depth > 0) return { open: i };
    if (text) parts.push({ text });
    text = "";
    parts.push({ command: written.slice(i + 2, j - 1) });
    i = j;
  }
  if (text) parts.push({ text });
  return parts;
}

/**
 * The commands whose output lands in the URL's user or password: between `://` and the `@` that
 * ends them. Their output is percent-encoded there, as a password with `/`, `@` or `?` in it, such
 * as a cloud's database token, would otherwise end the user part early; the drivers decode it. A
 * command elsewhere, such as one that prints the whole URL, is taken as printed.
 */
function inUserinfo(parts: UrlPart[]): Set<UrlPart> {
  let skeleton = "";
  const placed = new Map<number, UrlPart>();
  for (const part of parts) {
    if ("text" in part) skeleton += part.text;
    else {
      placed.set(skeleton.length, part);
      skeleton += "\0";
    }
  }
  const inside = new Set<UrlPart>();
  const scheme = skeleton.indexOf("://");
  if (scheme < 0) return inside;
  const start = scheme + 3;
  const ends = ["/", "?", "#"].map((d) => skeleton.indexOf(d, start)).filter((k) => k >= 0);
  const authorityEnd = Math.min(skeleton.length, ...ends);
  const userinfoEnd = skeleton.lastIndexOf("@", authorityEnd - 1);
  for (const [at, part] of placed) if (at >= start && at < userinfoEnd) inside.add(part);
  return inside;
}

/** What each command printed, by directory and command: run once per process, and again on a refused login or after a failure has stood a minute. */
const commandOutputs = new Map<string, { value: string } | { error: string; at: number }>();

/** What a command prints, its trailing newlines dropped, or why it printed nothing usable. */
function commandOutput(command: string, cwd: string, env: Record<string, string>, fresh: boolean): { value: string } | { error: string } {
  const key = `${cwd}\0${command}`;
  const kept = commandOutputs.get(key);
  if (!fresh && kept && ("value" in kept || Date.now() - kept.at < FAILED_COMMAND_TTL)) return kept;
  const run = spawnSync(command, { shell: true, cwd, env, encoding: "utf8", timeout: COMMAND_TIMEOUT_MS, stdio: ["ignore", "pipe", "pipe"], windowsHide: true });
  const said = run.stderr?.split(/\r?\n/).map((line) => line.trim()).find(Boolean)?.slice(0, 300);
  let outcome: { value: string } | { error: string };
  if (run.error) {
    const timedOut = (run.error as NodeJS.ErrnoException).code === "ETIMEDOUT";
    outcome = { error: timedOut ? `which did not finish within ${COMMAND_TIMEOUT_MS / 1000} s` : `which could not start: ${run.error.message}` };
  } else if (run.status !== 0) {
    outcome = { error: `which ${run.status === null ? `ended on ${run.signal}` : `exited with code ${run.status}`}${said ? `: ${said}` : ""}` };
  } else {
    const value = run.stdout.replace(/(\r?\n)+$/, "");
    outcome = value ? { value } : { error: "which printed nothing" };
  }
  commandOutputs.set(key, "value" in outcome ? outcome : { ...outcome, at: Date.now() });
  return outcome;
}

/**
 * The project `projectDir` belongs to. `engine` reads the project file: any database's engine does
 * (src/engine.ts, `anyEngine`), and where reylo has none to run a project file is the project's error.
 * With `urlGiven`, a caller has named its own database, so the project's connection is left unread,
 * and neither a command in `direct_url` nor a variable it names can fail.
 */
export function resolveProject(engine: Engine | MissingEngine, projectDir: string, env: Record<string, string | undefined> = process.env, options: { urlGiven?: boolean } = {}): ResolvedProject {
  const file = findProjectFile(projectDir, env);
  const root = file ? dirname(file) : packageDir(projectDir, env);
  const shown = (path: string) => (isAbsolute(path) ? relative(projectDir, path).split(sep).join("/") || path : path);
  let config: ProjectFile = {};
  let error: string | undefined;
  if (file) {
    let text: string | undefined;
    try {
      text = readFileSync(file, "utf8");
    } catch (e) {
      error = `Cannot read ${shown(file)}: ${(e as Error).message}`;
    }
    const types = text === undefined ? null : TYPES_KEY.exec(text);
    if (types) {
      error = `${shown(file!)}:${place(text!, Buffer.byteLength(text!.slice(0, types.index)))}: types: the types section was removed on 2026-10-20. Reylo now gives every value one type on every driver, whatever the driver's settings: a 64-bit integer is a BigInt, a decimal its exact text, a timestamptz its ISO 8601 text in UTC, a date and a timestamp their text, JSON parsed and binary data a Uint8Array. Delete the section.`;
    } else if (text !== undefined) {
      if (engine instanceof MissingEngine) error = `${shown(file)}: ${engine.message}`;
      else {
        const r = engine.call({ op: "config", text });
        if (r.status === "config") config = r.config as ProjectFile;
        else if (r.status === "configError") error = `${shown(file)}:${place(text, r.error.start)}: ${r.error.message}`;
        else error = `${shown(file)}: ${r.status === "error" ? r.message : `unexpected ${r.status}`}`;
      }
    }
  }
  // Both names beside one package.json: whichever were read, edits to the other would do nothing.
  const both = PROJECT_FILES.map((name) => join(root, name));
  if (!env.REYLO_CONFIG && both.every((path) => existsSync(path))) error = `${both.map(shown).join(" and ")} are both project files; keep one.`;
  const envFiles = (config.env ?? DEFAULT_ENV_FILES).map((f) => resolve(root, f));
  // The process environment wins; among the files, the first that sets a variable does.
  const lookup = (name: string): { value: string; source: string } | undefined => {
    if (env[name] !== undefined) return { value: env[name]!, source: `${name} in the environment` };
    for (const path of envFiles) {
      const value = readEnvFile(path)[name];
      if (value !== undefined) return { value, source: `${name} in ${shown(path)}` };
    }
    return undefined;
  };
  let url: string | undefined;
  let urlSource = "DATABASE_URL";
  // Reylo never reads `url`; a file that sets `direct_url` beside it has named Reylo's connection, and is fine.
  const urlAlone = config.url !== undefined && config.direct_url === undefined;
  const notice = urlAlone ? `${shown(file!)}: Reylo does not read url; set direct_url to the dev database Reylo reads your types from (it uses DATABASE_URL meanwhile)` : undefined;
  const missingUrl = urlAlone
    ? `No database URL: ${shown(file!)} sets url, which Reylo does not read; set direct_url there, or DATABASE_URL in the environment, .env.local or .env.`
    : `No database URL: set DATABASE_URL in the environment, .env.local or .env, or "direct_url" in ${file ? shown(file) : PROJECT_FILE}.`;
  const written = options.urlGiven ? undefined : config.direct_url;
  let refreshUrl: (() => string | undefined) | undefined;
  if (written !== undefined) {
    const missing: string[] = [];
    const sources: string[] = [];
    const parts = urlParts(written);
    urlSource = `"direct_url" in ${shown(file!)}`;
    if ("open" in parts) {
      error ??= `${shown(file!)}: "direct_url" opens $( at column ${parts.open + 1} and never closes it.`;
    } else {
      // The text's ${VAR}s are Reylo's; a command's are its shell's, so that no value is read as
      // shell syntax, and its shell sees the variables the text would: the environment's, then
      // the `.env` files'.
      const texts = parts.map((part) =>
        "text" in part
          ? interpolate(part.text, (name) => {
              const hit = lookup(name);
              if (hit?.value) sources.push(hit.source);
              return hit?.value;
            }, (name) => {
              const hit = lookup(name);
              if (!hit) missing.push(name);
              else sources.push(hit.source);
              return hit?.value ?? "";
            })
          : undefined,
      );
      urlSource += sources.length ? `, ${sources.join(", ")}` : "";
      if (missing.length) {
        error ??= `${shown(file!)}: "direct_url" names ${missing.map((m) => `\${${m}}`).join(", ")}, which neither the environment nor ${envFiles.map(shown).join(" nor ")} sets.`;
      } else if (parts.some((part) => "command" in part)) {
        const shellEnv: Record<string, string> = {};
        for (const path of [...envFiles].reverse()) Object.assign(shellEnv, readEnvFile(path));
        for (const [name, value] of Object.entries({ ...process.env, ...env })) if (value !== undefined) shellEnv[name] = value;
        const encoded = inUserinfo(parts);
        const build = (fresh: boolean): { url: string } | { error: string } => {
          let built = "";
          for (const [i, part] of parts.entries()) {
            if ("text" in part) {
              built += texts[i];
              continue;
            }
            const out = commandOutput(part.command, root, shellEnv, fresh);
            if ("error" in out) return { error: `${shown(file!)}: "direct_url" runs \`${part.command}\`, ${out.error}.` };
            built += encoded.has(part) ? encodeURIComponent(out.value) : out.value;
          }
          return { url: built };
        };
        // A project that cannot be used anyway runs nothing, so that it asks for no password.
        if (!error) {
          const first = build(false);
          if ("error" in first) error = first.error;
          else url = first.url;
          refreshUrl = () => {
            const again = build(true);
            return "url" in again ? again.url : undefined;
          };
        }
      } else url = texts.join("");
    }
  } else if (!options.urlGiven) {
    const hit = lookup("DATABASE_URL");
    url = hit?.value;
    if (hit) urlSource = hit.source;
  }
  let dialect = url ? dialectOf(url) : "postgres";
  if (url && !dialect) error ??= `Cannot tell the database from the URL (${urlSource}): reylo reads postgres://, postgresql://, mysql://, file:, sqlite: and duckdb: URLs.`;
  // Either name, created or removed, changes the project.
  const watched = [...new Set([...(file ? [file] : []), ...both]), ...envFiles];
  // No URL names the database: the entries of the cache, which a build with no URL reads
  // (src/analyze.ts), do where they are all of one dialect.
  const cache = join(resolve(root, config.dir ?? "reylo"), ".cache");
  const cached = url ? [] : cachedDialects(cache);
  if (cached.length === 1) dialect = cached[0];
  return { file, root, cache, dialect: dialect ?? "postgres", strictParams: !(config.allow_untyped_params ?? false), allowNestedArrays: config.allow_nested_arrays ?? false, sqlcommenter: config.sqlcommenter ?? true, codecs: config.js?.codecs ?? {}, url, urlSource, refreshUrl, watched, error, notice, missingUrl };
}

/** `duckdb:./dev.duckdb`, `duckdb:///abs/dev.duckdb` → a path, relative ones resolved against baseDir; `duckdb::memory:` (or `duckdb:` alone) → `:memory:`, an empty database, for statements that read files alone. */
export function duckdbPath(url: string, baseDir: string): string {
  let raw = url.replace(/^duckdb:(\/\/)?/i, "");
  if (raw === "" || raw === ":memory:") return ":memory:";
  // On Windows, `duckdb:///C:/app.duckdb` names the drive, as a file URL does.
  if (process.platform === "win32") raw = raw.replace(/^\/(?=[A-Za-z]:[\\/])/, "");
  return isAbsolute(raw) ? raw : resolve(baseDir, raw);
}

/** `file:./dev.db`, `file:///abs/dev.db`, `sqlite:./dev.db`, `:memory:` → a path, relative ones resolved against baseDir. */
export function sqlitePath(url: string, baseDir: string): string {
  if (url === ":memory:" || url === "file::memory:") return ":memory:";
  const raw = url.startsWith("file://") ? fileURLToPath(url) : url.replace(/^(file|sqlite):(\/\/)?/, "");
  return isAbsolute(raw) ? raw : resolve(baseDir, raw);
}
