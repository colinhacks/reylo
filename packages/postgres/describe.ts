// The JavaScript half of a Postgres analysis, which this package carries with postgres.js inside
// it (scripts/build.ts bundles them into describe.js): Parse/Describe and the engine's catalog
// queries, so the type check describes the same way whatever driver the app runs on. The analysis
// loads it by its database (src/engine.ts, `describer`).
import { readFileSync } from "node:fs";
import type { ConnectionOptions } from "node:tls";
import postgres from "postgres";
import { Unreachable } from "../../src/connection.ts";

export type Described = {
  params: { oid: number }[];
  columns: { name: string; oid: number; table: number; attnum: number }[];
};

export interface Describer {
  describe(sql: string): Promise<Described>;
  query(sql: string, params?: unknown[]): Promise<Record<string, unknown>[]>;
  /** Rows as arrays of values, in column order: what the engine's catalog queries read. */
  values(sql: string, params?: unknown[]): Promise<unknown[][]>;
  close(): Promise<void>;
}

/**
 * The server's answer to a statement carries a severity (pg's DatabaseError, postgres.js's
 * PostgresError). Anything else is the connection: a socket error, a timeout, a closed client.
 * So is a FATAL or PANIC error and the connection-exception class 08, after which the server
 * has closed the session (a database that does not exist is a FATAL 3D000).
 */
function settled(e: unknown): Error {
  const err = e as { severity?: string; code?: string };
  return err?.severity && err.severity !== "FATAL" && err.severity !== "PANIC" && !err.code?.startsWith("08") ? (e as Error) : new Unreachable(e);
}

/** How long a connection attempt may take: the transform waits on it, in the editor and in CI. */
const CONNECT_TIMEOUT_SECONDS = 10;

/**
 * postgres.js sends a URL parameter it does not know to the server as a setting, and the server
 * refuses it. Two kinds of parameter reach this far. `channel_binding`, which Neon's URLs carry:
 * postgres.js cannot bind the channel either, so a URL that requires it has the server's
 * certificate verified instead. And libpq's TLS files (`sslrootcert`, `sslcert`, `sslkey`,
 * `sslpassword`), which postgres.js reads only as `sslrootcert=system`: they become the TLS
 * options of the connection, verified as libpq does: `verify-ca`, `verify-full` and a root
 * certificate file check the certificate chain, and `verify-full` and `sslrootcert=system` the host
 * name too; without a root certificate file, `require` checks nothing.
 */
export function forPostgresJs(url: string): { url: string; ssl?: "verify-full" | ConnectionOptions } {
  const q = url.indexOf("?");
  if (q < 0) return { url };
  const params = new URLSearchParams(url.slice(q + 1));
  const binding = params.get("channel_binding");
  const files = ["sslrootcert", "sslcert", "sslkey", "sslpassword"].filter((k) => params.has(k) && !(k === "sslrootcert" && params.get(k) === "system"));
  if (binding === null && files.length === 0) return { url };
  const mode = params.get("sslmode");
  const read = (key: string) => (files.includes(key) ? readFileSync(params.get(key)!) : undefined);
  // As libpq: a root certificate file makes `require` check the chain like `verify-ca`, and
  // `sslrootcert=system` checks the chain and the host name like `verify-full`.
  const system = params.get("sslrootcert") === "system";
  const chain = mode === "verify-ca" || mode === "verify-full" || binding === "require" || system || files.includes("sslrootcert");
  const host = mode === "verify-full" || system || binding === "require";
  const options: ConnectionOptions | undefined = files.length > 0 && mode !== "disable"
    ? { ca: read("sslrootcert"), cert: read("sslcert"), key: read("sslkey"), passphrase: params.get("sslpassword") ?? undefined, rejectUnauthorized: chain, ...(chain && !host && { checkServerIdentity: () => undefined }) }
    : undefined;
  for (const key of ["channel_binding", ...files]) params.delete(key);
  const rest = params.toString();
  const stripped = url.slice(0, q) + (rest ? `?${rest}` : "");
  if (options) return { url: stripped, ssl: options };
  return binding === "require" ? { url: stripped, ssl: "verify-full" } : { url: stripped };
}

/**
 * An array parameter as the text of an array literal, which the query's own cast
 * (`$1::pg_catalog.text[]`) reads: postgres.js serializes an array by the array types it reads from
 * the catalog when it connects, and the connection reads none (below).
 */
export function arrayLiteral(value: unknown): unknown {
  if (!Array.isArray(value)) return value;
  const item = (x: unknown): string =>
    Array.isArray(x) ? (arrayLiteral(x) as string) : x === null || x === undefined ? "NULL" : typeof x === "boolean" ? (x ? "t" : "f") : `"${String(x).replace(/[\\"]/g, "\\$&")}"`;
  return `{${value.map(item).join(",")}}`;
}

/** How postgres.js reads an element of each built-in array type a catalog query answers. */
const ELEMENTS: Record<number, (x: string) => unknown> = {
  1000: (x) => x === "t",
  1005: Number,
  1007: Number,
  1028: Number,
  1021: Number,
  1022: Number,
  1016: String,
  1009: String,
  1015: String,
  1003: String,
  1002: String,
  1014: String,
};

/**
 * The text of a one-dimensional or nested array of `oid`'s type as values: an unquoted `NULL` is
 * null, and a quoted element has its backslash escapes undone.
 */
export function parseArray(text: string, element: (x: string) => unknown): unknown {
  let i = text.startsWith("[") ? text.indexOf("=") + 1 : 0;
  const read = (): unknown[] => {
    const out: unknown[] = [];
    i++; // "{"
    if (text[i] === "}") {
      i++;
      return out;
    }
    for (;;) {
      if (text[i] === "{") out.push(read());
      else if (text[i] === '"') {
        let value = "";
        for (i++; text[i] !== '"'; i++) value += text[i] === "\\" ? text[++i] : text[i];
        i++;
        out.push(element(value));
      } else {
        const end = Math.min(...[",", "}"].map((c) => text.indexOf(c, i)).filter((n) => n >= 0));
        const raw = text.slice(i, end).trim();
        i = end;
        out.push(raw === "NULL" ? null : element(raw));
      }
      if (text[i++] === "}") return out;
    }
  };
  return read();
}

/** Rows, as objects or as arrays, with each column of a built-in array type read as an array. */
function withArrays<R extends readonly unknown[]>(rows: R, key: (column: { name: string }, i: number) => string | number): R {
  const columns: { name: string; type: number }[] = (rows as { columns?: { name: string; type: number }[] }).columns ?? [];
  const arrays = columns.map((c, i) => [key(c, i), ELEMENTS[c.type]] as const).filter((a): a is readonly [string | number, (x: string) => unknown] => a[1] !== undefined);
  for (const row of rows as unknown as Record<string | number, unknown>[]) {
    for (const [k, element] of arrays) if (typeof row[k] === "string") row[k] = parseArray(row[k] as string, element);
  }
  return rows;
}

export async function openPostgresDescriber(url: string): Promise<Describer> {
  const target = forPostgresJs(url);
  // `fetch_types` off: postgres.js would read the catalog's array types at connect with a query of
  // bare operators (`b.oid = a.typelem`), which a user's `=` captures under a search path that puts
  // their schema before pg_catalog, and arrays would then travel as text both ways. The catalog
  // queries' arrays are read and written here instead (`arrayLiteral`, `parseArray`).
  const sql = postgres(target.url, { max: 1, prepare: false, fetch_types: false, onnotice: () => {}, connect_timeout: CONNECT_TIMEOUT_SECONDS, ...(target.ssl && { ssl: target.ssl }) });
  // postgres.js runs these queries one at a time and connects again for each one queued behind
  // a failure, so five transforms against a host that never answers would wait five timeouts
  // (ending the client does not release the queue). The first failure answers every call that
  // is waiting and every call after it.
  let failure: Error | undefined;
  let abort!: (e: Error) => void;
  const aborted = new Promise<never>((_, reject) => { abort = reject; });
  aborted.catch(() => {});
  const guard = <T>(run: () => PromiseLike<T>): Promise<T> => {
    if (failure) return Promise.reject(failure);
    return Promise.race([run(), aborted]).catch((e: unknown): never => {
      if (failure) throw failure;
      const err = settled(e);
      if (err instanceof Unreachable) {
        failure = err;
        abort(err);
        sql.end({ timeout: 0 }).catch(() => {});
      }
      throw err;
    });
  };
  return {
    async describe(text) {
      const d = await guard<any>(() => sql.unsafe(text).describe());
      return {
        params: (d.types as number[]).map((oid) => ({ oid })),
        columns: (d.columns as any[]).map((c) => ({ name: c.name, oid: c.type, table: c.table, attnum: c.number })),
      };
    },
    query: (text, params = []) => guard(async () => withArrays(await sql.unsafe(text, params.map(arrayLiteral) as any[]), (c) => c.name)),
    values: (text, params = []) => guard(async () => withArrays(await sql.unsafe(text, params.map(arrayLiteral) as any[]).values(), (_, i) => i)),
    close: () => sql.end({ timeout: 0 }),
  };
}
