// The Postgres host on PGlite, in the same thread as the engine: a describe is Parse, Describe and
// Sync sent as one execProtocol call (its RowDescription carries each column's table oid and
// column number, which the engine's nullability reads); a catalog query is `query` with rows as
// arrays. PGlite is one connection, and execProtocol is not queued behind `query`, so every call
// takes its turn here.
import type { PGlite } from "@electric-sql/pglite";
import type { EngineIo } from "../../../src/engine-core.ts";

const enc = new TextEncoder();
function message(type: string, body: Uint8Array): Uint8Array {
  const b = new Uint8Array(5 + body.length);
  b[0] = type.charCodeAt(0);
  new DataView(b.buffer).setInt32(1, 4 + body.length);
  b.set(body, 5);
  return b;
}
function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) (out.set(p, at), (at += p.length));
  return out;
}
const cstr = (s: string) => concat([enc.encode(s), new Uint8Array([0])]);
/** Parse (unnamed, no parameter types) + Describe 'S' + Sync. */
export function describeMessage(text: string): Uint8Array {
  return concat([
    message("P", concat([cstr(""), cstr(text), new Uint8Array([0, 0])])),
    message("D", new Uint8Array([83, 0])),
    message("S", new Uint8Array()),
  ]);
}

/** postgres.js's `.values()` gives JSON-safe values; PGlite gives an int8 as a BigInt past 2^53. */
function jsonSafe(v: unknown): unknown {
  if (typeof v === "bigint") return v.toString();
  if (Array.isArray(v)) return v.map(jsonSafe);
  return v;
}

export function pgliteIo(pg: PGlite): EngineIo {
  let turn: Promise<unknown> = Promise.resolve();
  const serial = <T>(call: () => Promise<T>): Promise<T> => {
    const next = turn.then(call, call);
    turn = next.catch(() => {});
    return next;
  };
  return {
    describe(text) {
      return serial(async () => {
        const { messages } = await pg.execProtocol(describeMessage(text), { throwOnError: false });
        const out = { params: [] as { oid: number }[], columns: [] as { name: string; oid: number; table: number; attnum: number }[] };
        for (const m of messages as any[]) {
          if (m.name === "error") {
            const err = new Error(m.message) as Error & { code?: string; position?: string };
            err.code = m.code;
            if (m.position !== undefined) err.position = m.position;
            throw err;
          }
          if (m.name === "parameterDescription") out.params = (m.dataTypeIDs as number[]).map((oid) => ({ oid }));
          if (m.name === "rowDescription") out.columns = (m.fields as any[]).map((f) => ({ name: f.name, oid: f.dataTypeID, table: f.tableID, attnum: f.columnID }));
        }
        return out;
      });
    },
    query(text, params) {
      return serial(async () => {
        const r = await pg.query<unknown[]>(text, params as any[], { rowMode: "array" });
        return r.rows.map((row) => (row as unknown[]).map(jsonSafe));
      });
    },
  };
}
