// What a SQLite-backed Durable Object's SQL storage, `ctx.storage.sql`, takes and gives, in workerd
// through Miniflare: the facts Reylo's Durable Object path (src/execute.ts, src/decode-sqlite.ts)
// and the fake in tests/execute.test.ts rest on. Run with ./setup.sh done: node probes/durable-object.mjs
//
// Measured with miniflare 4.20260730.0:
// - exec(sql, ...bindings) runs at once and returns a cursor: toArray(), one(), raw(), columnNames,
//   rowsRead, rowsWritten. The storage has exec(), databaseSize, Cursor and Statement, and no
//   prepare(), all(), execute() or unsafe(). exec() runs several statements, and a write gives [].
// - A BigInt binding throws "Cannot convert a BigInt value to a number". `true` is bound as the
//   text "true", undefined as NULL, a Uint8Array or an ArrayBuffer as a BLOB.
// - An INTEGER past 2^53 comes back as a rounded number, with no error. A BLOB is an ArrayBuffer.
// - toArray() keeps the last of two columns of one name; raw() keeps both. A row is a plain object.
// - The wrong number of bindings, BEGIN and SAVEPOINT are errors; the freshness check's
//   pragma_table_xinfo query and sqlite_master read as on any SQLite.
import { createRequire } from "node:module";

const require = createRequire(new URL("../package.json", import.meta.url));
const { Miniflare } = require("miniflare-4");

const script = `
export class Probe {
  constructor(ctx) { this.ctx = ctx; }
  async fetch() {
    const sql = this.ctx.storage.sql;
    const out = {};
    const shown = (v) => v === null ? "null" : v instanceof ArrayBuffer ? "ArrayBuffer(" + v.byteLength + ")" : typeof v + ":" + String(v);
    const probe = (name, f) => { try { out[name] = f(); } catch (e) { out[name] = "throws " + e.constructor.name + ": " + e.message; } };
    sql.exec("create table t (id integer primary key, u text unique)");
    sql.exec("create table users (id integer primary key, email text not null)");
    probe("storage", () => ({ own: Object.getOwnPropertyNames(Object.getPrototypeOf(sql)), prepare: typeof sql.prepare, all: typeof sql.all }));
    probe("cursor", () => { const c = sql.exec("select 1 as a, 2 as a"); return { own: Object.getOwnPropertyNames(Object.getPrototypeOf(c)), columnNames: c.columnNames, raw: [...c.raw()] }; });
    probe("a write's toArray()", () => sql.exec("insert into t (u) values (?)", "a").toArray());
    probe("two columns of one name", () => sql.exec("select 1 as a, 2 as a").toArray());
    probe("bind a number", () => shown(sql.exec("select ? as v", 5).one().v));
    probe("bind a BigInt", () => shown(sql.exec("select ? as v", 5n).one().v));
    probe("bind true", () => shown(sql.exec("select ? as v", true).one().v));
    probe("bind undefined", () => shown(sql.exec("select ? as v", undefined).one().v));
    probe("bind a Uint8Array", () => shown(sql.exec("select ? as v", new Uint8Array([1, 2])).one().v));
    probe("read 2^53 + 1", () => shown(sql.exec("select 9007199254740993 as v").one().v));
    probe("read a BLOB", () => shown(sql.exec("select x'0102' as v").one().v));
    probe("too few bindings", () => sql.exec("select ? as a, ? as b", 1).toArray());
    probe("unique", () => sql.exec("insert into t (u) values ('a')").toArray());
    probe("begin", () => sql.exec("begin").toArray());
    probe("several statements", () => { sql.exec("create table m (a); insert into m values (1)"); return sql.exec("select count(*) as n from m").one().n; });
    probe("sqlite_master", () => sql.exec("select count(*) as n from sqlite_master where type = 'table' and name = 'users'").one().n);
    probe("pragma_table_xinfo", () => sql.exec("select p.name, p.type, p.[notnull] from pragma_table_xinfo('users') p order by 1").toArray());
    probe("a row's prototype", () => Object.getPrototypeOf(sql.exec("select 1 as a").toArray()[0]) === Object.prototype);
    return Response.json(out);
  }
}
export default { fetch(request, env) { return env.PROBE.get(env.PROBE.idFromName("probe")).fetch(request); } };`;

// The newest compatibility date the matrix's workerd supports.
const mf = new Miniflare({ modules: true, script, compatibilityDate: "2026-08-01", durableObjects: { PROBE: { className: "Probe", useSQLite: true } } });
try {
  const facts = await (await mf.dispatchFetch("http://probe/")).json();
  for (const [name, value] of Object.entries(facts)) console.log(`${name}: ${JSON.stringify(value)}`);
} finally {
  await mf.dispose();
}
