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

export async function openPostgresDescriber(url: string): Promise<Describer> {
  const target = forPostgresJs(url);
  const sql = postgres(target.url, { max: 1, prepare: false, onnotice: () => {}, connect_timeout: CONNECT_TIMEOUT_SECONDS, ...(target.ssl && { ssl: target.ssl }) });
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
    query: (text, params = []) => guard(() => sql.unsafe(text, params as any[])),
    values: (text, params = []) => guard(() => sql.unsafe(text, params as any[]).values()),
    close: () => sql.end({ timeout: 0 }),
  };
}
