// SQLite compiled to WebAssembly, on a copy of the database file the runner seeded (--file): in
// Node neither opens a file, so the file's bytes are read into a database in memory, as a browser
// opens one it was sent.
//   --lib sqlite-wasm   SQLite Wasm's oo1 Database (@sqlite.org/sqlite-wasm), and the same inside
//                       BEGIN/COMMIT by exec(). Its db.transaction(fn) calls the function at once,
//                       where better-sqlite3's returns one to call, so the syncTx scenarios do not
//                       apply; tests/execute.test.ts rolls one back.
//   --lib sql.js        sql.js's Database, and the same inside BEGIN/COMMIT by exec(). Before
//                       1.17.0 reylo refuses it: it threw SQLite's errors with no code and bound
//                       a BigInt as text.
import { readFileSync } from "node:fs";
import { args, manualTx, notRun, requireFrom, runJob, type Shape } from "../lib/harness.ts";

const a = args();
const wasm = a.lib === "sqlite-wasm";
const meta = { pkg: wasm ? "@sqlite.org/sqlite-wasm" : "sql.js", version: a.version!, target: "SQLite file, copied into memory" };

let open: (bytes: Uint8Array) => any;
try {
  if (wasm) {
    const sqlite3 = await (await import(requireFrom.resolve("@sqlite.org/sqlite-wasm"))).default();
    const { capi } = sqlite3;
    open = (bytes) => {
      const db = new sqlite3.oo1.DB();
      const at = sqlite3.wasm.allocFromTypedArray(bytes);
      db.checkRc(capi.sqlite3_deserialize(db.pointer, "main", at, bytes.length, bytes.length, capi.SQLITE_DESERIALIZE_FREEONCLOSE | capi.SQLITE_DESERIALIZE_RESIZEABLE));
      return db;
    };
  } else {
    const SQL = await requireFrom(a.pkg ?? "sql.js")();
    open = (bytes) => new SQL.Database(bytes);
  }
} catch (e) {
  notRun(meta, `cannot load ${meta.pkg}: ${(e as Error).message.split("\n")[0]}`);
  process.exit(0);
}

await runJob(meta, async () => {
  const db = open(new Uint8Array(readFileSync(a.file!)));
  const [major, minor] = a.version!.split(".").map(Number);
  const refused = !wasm && major! === 1 && minor! < 17 ? /reylo takes sql\.js 1\.17\.0 or later/ : undefined;
  const name = wasm ? "oo1.DB" : "Database";
  const shapes: Shape[] = [
    { name, kind: "sync", scope: (fn) => fn(db), ...(refused && { refused }) },
  ];
  if (!refused) shapes.push(
    {
      name: `${name} in BEGIN/COMMIT (exec)`,
      kind: "sync",
      inTransaction: true,
      scope: async (fn) => {
        db.exec("BEGIN");
        try {
          await fn(db);
          db.exec("COMMIT");
        } catch (e) {
          db.exec("ROLLBACK");
          throw e;
        }
      },
      tx: manualTx({
        open: async () => ({ client: db }),
        begin: (c) => c.exec("BEGIN"),
        commit: (c) => c.exec("COMMIT"),
        rollback: (c) => c.exec("ROLLBACK"),
        exec: (c, sql) => c.exec(sql),
      }),
    },
  );
  const count = wasm ? async (tag: string) => Number(db.selectValue("select count(*) from scratch where tag = ?", [tag])) : async (tag: string) => Number(db.exec("select count(*) from scratch where tag = ?", [tag])[0].values[0][0]);
  return { ...meta, dialect: "sqlite", count, shapes, close: async () => db.close() };
});
