// Writes the three lists of error codes `.try()` names (src/query-error.ts), each from its
// database's own source:
// - src/error-codes-postgres.ts: every SQLSTATE Postgres raises as an error and its condition name,
//   from `src/backend/utils/errcodes.txt` at the tag below;
// - src/error-codes-mysql.ts: every error number MySQL sends a client and its name, from
//   `share/messages_to_clients.txt` at the tag below, which the server's `mysqld_error.h` is built
//   from;
// - src/error-codes-sqlite.ts: every primary and extended result code of an error and its name, from
//   the `sqlite3.h` of the amalgamation `engine/build.sh --prepare` fetches.
// Run it as `nub scripts/error-codes.ts` after moving a tag, and commit what it writes.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const POSTGRES_TAG = "REL_18_0";
const MYSQL_TAG = "mysql-8.4.11";

const root = join(import.meta.dirname, "..");

async function fetchText(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  return response.text();
}

// errcodes.txt: `<sqlstate> <E|W|S> ERRCODE_<macro> [<condition name>]`, a code listed twice where
// two macros share it, one line of the two naming its condition.
const postgres = new Map<string, string>();
for (const line of (await fetchText(`https://raw.githubusercontent.com/postgres/postgres/${POSTGRES_TAG}/src/backend/utils/errcodes.txt`)).split("\n")) {
  const m = /^([0-9A-Z]{5})\s+E\s+ERRCODE_\w+(?:\s+(\w+))?\s*$/.exec(line);
  if (!m) continue;
  const [, code, condition] = m;
  if (condition) postgres.set(code!, condition);
  else if (!postgres.has(code!)) postgres.set(code!, "");
}
for (const [code, condition] of postgres) if (!condition) throw new Error(`errcodes.txt names no condition for ${code}`);

// messages_to_clients.txt: `start-error-number <n>` sets the next number, each entry is a line
// starting with its name (and its SQLSTATE), and the lines under it are its messages. An
// `OBSOLETE_` entry keeps its number and is no longer sent.
const mysql: [number, string][] = [];
let next = 0;
for (const line of (await fetchText(`https://raw.githubusercontent.com/mysql/mysql-server/${MYSQL_TAG}/share/messages_to_clients.txt`)).split("\n")) {
  const start = /^start-error-number (\d+)/.exec(line);
  if (start) {
    next = Number(start[1]);
    continue;
  }
  const entry = /^((?:ER|WARN|OBSOLETE)_\w+)\b/.exec(line);
  if (!entry) continue;
  if (!next) throw new Error(`messages_to_clients.txt: ${entry[1]} before any start-error-number`);
  if (!entry[1]!.startsWith("OBSOLETE_")) mysql.push([next, entry[1]!]);
  next++;
}

// sqlite3.h: `#define SQLITE_<name> <n>` for a primary code and `(SQLITE_<primary> | (<k><<8))` for an
// extended one, under "Result Codes" and "Extended Result Codes". SQLITE_OK, SQLITE_ROW and
// SQLITE_DONE, and the extended codes of SQLITE_OK, are no error.
const amalgamation = join(root, "engine/sqlite/.sqlite");
const dir = readdirSync(amalgamation).find((d) => d.startsWith("sqlite-amalgamation-"));
if (!dir) throw new Error(`${amalgamation} holds no amalgamation: run engine/build.sh --prepare`);
const header = readFileSync(join(amalgamation, dir, "sqlite3.h"), "utf8");
const sqliteVersion = /#define SQLITE_VERSION\s+"([\d.]+)"/.exec(header)![1]!;
const section = header.slice(header.indexOf("CAPI3REF: Result Codes"), header.indexOf("CAPI3REF: Flags For File Open Operations"));
const sqlite = new Map<string, number>();
for (const m of section.matchAll(/^#define (SQLITE_\w+)\s+(?:(\d+)|\((SQLITE_\w+)\s*\|\s*\((\d+)<<8\)\))/gm)) {
  const [, name, value, primary, shift] = m;
  sqlite.set(name!, value !== undefined ? Number(value) : sqlite.get(primary!)! | (Number(shift) << 8));
}
const noError = (code: number) => [0, 100, 101].includes(code & 0xff);
const sqliteCodes = [...sqlite].filter(([, code]) => !noError(code)).sort((a, b) => a[1] - b[1]);

// The MySQL list as runs of consecutive numbers, which keeps the module small.
const runs: [number, string[]][] = [];
for (const [errno, name] of mysql) {
  const last = runs.at(-1);
  if (last && last[0] + last[1].length === errno) last[1].push(name);
  else runs.push([errno, [name]]);
}

const banner = (source: string) => `// Written by scripts/error-codes.ts from ${source}; run it again rather than editing this file.\n`;
writeFileSync(
  join(root, "src/error-codes-postgres.ts"),
  banner(`Postgres's src/backend/utils/errcodes.txt at ${POSTGRES_TAG}`) +
    `\n/** Every SQLSTATE Postgres raises as an error, with its condition name. */\n` +
    `export const POSTGRES_CODES = {\n${[...postgres].sort(([a], [b]) => (a < b ? -1 : 1)).map(([code, condition]) => `  "${code}": "${condition}",`).join("\n")}\n} as const;\n`,
);
writeFileSync(
  join(root, "src/error-codes-mysql.ts"),
  banner(`MySQL's share/messages_to_clients.txt at ${MYSQL_TAG}`) +
    `\n/** Every error number MySQL sends a client and its name, as runs of consecutive numbers: \`[first, names]\`. */\n` +
    `export const MYSQL_CODES = [\n${runs.map(([first, names]) => `  [${first}, [${names.map((n) => `"${n}"`).join(", ")}]],`).join("\n")}\n] as const;\n`,
);
writeFileSync(
  join(root, "src/error-codes-sqlite.ts"),
  banner(`the sqlite3.h of SQLite ${sqliteVersion}`) +
    `\n/** Every primary and extended result code of an error, by name. */\n` +
    `export const SQLITE_CODES = {\n${sqliteCodes.map(([name, code]) => `  ${name}: ${code},`).join("\n")}\n} as const;\n`,
);
console.log(`postgres ${postgres.size} codes, mysql ${mysql.length} numbers in ${runs.length} runs, sqlite ${sqliteCodes.length} codes`);
