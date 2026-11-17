import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { DRIVERS, INDEX, ROOT, declarations, sqlserverDatabase, sqlserverLive, transform } from "./helpers.ts";

// SQL Server end to end, on the throwaway server REYLO_TEST_SQLSERVER names, each test in a database
// of its own: the types the content mapper writes for a statement, read from the server, and the
// rows its module's `.run()` returns on `mssql` (lab/drivers), one representation per SQL type
// (src/values.ts). A decimal's and a datetimeoffset's exact value needs tedious's `decimalAsString`
// and `offsetMinutes`: where the installed tedious has them each test checks the values, and where
// it lacks them, the refusal that names them.

function project(url: string): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-sqlserver-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  mkdirSync(join(root, "node_modules"), { recursive: true });
  symlinkSync(join(DRIVERS, "mssql"), join(root, "node_modules", "mssql"));
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  return root;
}

/** The module a build step writes for `sql` in `root`, imported. */
async function built(root: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  try {
    const { code } = await sqlModule(file, sql);
    const out = join(root, `${name}.mjs`);
    writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
    return (await import(pathToFileURL(out).href)).default;
  } finally {
    for (const session of Session.opened()) await session.close();
  }
}

/** The `Params` and `Row` a declaration exports. */
function types(text: string): { params: string; row: string } {
  const params = /^export type Params = ([\s\S]*?);\nexport type Row/m.exec(text)![1]!;
  const row = /^export type Row = ([\s\S]*?);\n(?=\/\*\*|export )/m.exec(text)![1]!;
  return { params, row };
}

/** Rows compared by value: a Buffer is a Uint8Array, and no value Reylo returns is a Date. */
const plain = (v: unknown): unknown => {
  if (v instanceof Date) throw new assert.AssertionError({ message: `a Date in Reylo's rows: ${v.toISOString()}` });
  return v instanceof Uint8Array ? { bytes: [...v] } : Array.isArray(v) ? v.map(plain) : v !== null && typeof v === "object" ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, plain(x)])) : v;
};

/** Whether the installed tedious reads a decimal as its exact text with `decimalAsString`, and gives a datetimeoffset's offset. */
async function exactDriver(db: { pool: any }): Promise<boolean> {
  const result = await db.pool.request().query("select cast(1.50 as decimal(3, 2)) as d, cast('2026-01-02T03:04:05+05:30' as datetimeoffset(0)) as o");
  const row = result.recordset[0];
  return typeof row.d === "string" && typeof row.o?.offsetMinutes === "number";
}

const SCHEMA = `create table t (
  id int not null primary key,
  flag bit not null,
  tiny tinyint null, small smallint null, big bigint null,
  amount decimal(12, 4) null, price money null, ratio float null, single real null,
  code char(3) null, label varchar(20) null, title nvarchar(40) null, body nvarchar(max) null,
  raw varbinary(8) null, uid uniqueidentifier null,
  day date null, at time(3) null, stamp datetime2(7) null, zoned datetimeoffset(3) null, legacy datetime null, minute smalldatetime null
);
insert into t values (1, 1, 255, -32768, 9007199254740993, 12345678.1234, 922337203685477.5807, 1.5, 0.25, 'abc', 'hello', N'héllo 😀', N'long', 0x00ff, '6F9619FF-8B86-D011-B42D-00C04FC964FF',
  '0001-01-01', '12:34:56.789', '9999-12-31T23:59:59.9999999', '2026-01-02T03:04:05.123+05:30', '2026-01-02T03:04:05.997', '2026-01-02T03:04:00');
insert into t (id, flag) values (2, 0);`;

const ROW_1 = {
  id: 1, flag: true, tiny: 255, small: -32768, big: 9007199254740993n, amount: "12345678.1234", price: "922337203685477.5807", ratio: 1.5, single: 0.25,
  code: "abc", label: "hello", title: "héllo 😀", body: "long", raw: { bytes: [0, 255] }, uid: "6F9619FF-8B86-D011-B42D-00C04FC964FF",
  day: "0001-01-01", at: "12:34:56.789", stamp: "9999-12-31T23:59:59.9999999", zoned: "2026-01-02T03:04:05.123+05:30", legacy: "2026-01-02T03:04:05.997", minute: "2026-01-02T03:04:00",
};
const ROW_2 = Object.fromEntries(Object.keys(ROW_1).map((k) => [k, k === "id" ? 2 : k === "flag" ? false : null]));

test("SQL Server: a select of every type is typed from the server, and runs on mssql's pool, a transaction and a request", { skip: sqlserverLive, timeout: 120_000 }, async () => {
  const db = await sqlserverDatabase("reylo_sqlserver_types", SCHEMA, { decimalAsString: true });
  try {
    const root = project(db.url);
    const sql = "select * from t where id = :id";
    const out = await declarations(root, { "row.sql": sql });
    assert.deepEqual(types(out["row.sql"]!), {
      params: "{ id: number }",
      row: "{ id: number; flag: boolean; tiny: number | null; small: number | null; big: bigint | null; amount: string | null; price: string | null; ratio: number | null; single: number | null; code: string | null; label: string | null; title: string | null; body: string | null; raw: Uint8Array | null; uid: string | null; day: string | null; at: string | null; stamp: string | null; zoned: string | null; legacy: string | null; minute: string | null }",
    });
    const q = await built(root, "row", sql);
    // T-SQL's own `@name` is a placeholder too, bound by the same name.
    const native = await built(root, "native", "select id, flag, big from t where id = @id");
    assert.deepEqual(plain(await native.run(db.pool, { id: 1 })), { id: 1, flag: true, big: 9007199254740993n });
    if (await exactDriver(db)) {
      assert.deepEqual(plain(await q.run(db.pool, { id: 1 })), ROW_1);
      assert.deepEqual(plain(await q.run(db.pool, { id: 2 })), ROW_2);
      assert.equal(await q.run(db.pool, { id: 3 }), null);
      const tx = new db.mssql.Transaction(db.pool);
      await tx.begin();
      assert.deepEqual(plain(await q.run(tx, { id: 1 })), ROW_1, "in a transaction");
      await tx.rollback();
      assert.deepEqual(plain(await q.run(new db.mssql.Request(db.pool), { id: 1 })), ROW_1, "on a request");
    } else {
      await assert.rejects(q.run(db.pool, { id: 1 }), /Column "amount" is a SQL Server decimal, which tedious gave as 12345678\.1234: Reylo returns its exact text, which tedious gives with the pool's options\.decimalAsString set to true\./);
    }
    // A pool that reads decimals as numbers is refused, never rounded.
    const loose = await db.connect({ decimalAsString: false });
    try {
      await assert.rejects(q.run(loose, { id: 1 }), /Column "amount" is a SQL Server decimal, which tedious gave as 12345678\.1234/);
      assert.deepEqual(plain(await built(root, "small", "select id, flag, big from t where id = :id").then((s) => s.run(loose, { id: 1 }))), { id: 1, flag: true, big: 9007199254740993n });
    } finally {
      await loose.close();
    }
    // A pool that reads dates in the process's time zone is refused.
    const local = await db.connect({ useUTC: false });
    try {
      await assert.rejects(q.run(local, { id: 1 }), /reylo reads SQL Server's dates and times in UTC/);
    } finally {
      await local.close();
    }
  } finally {
    await db.drop();
  }
});

test("SQL Server: each parameter is bound as the server typed it, and comes back as written", { skip: sqlserverLive, timeout: 120_000 }, async () => {
  const db = await sqlserverDatabase("reylo_sqlserver_params", SCHEMA, { decimalAsString: true });
  try {
    const root = project(db.url);
    const insert = "insert into t (id, flag, big, amount, label, title, raw, uid, day, at, stamp, zoned) output inserted.id values (:id, :flag, :big, :amount, :label, :title, :raw, :uid, :day, :at, :stamp, :zoned)";
    const out = await declarations(root, { "insert.sql": insert });
    assert.equal(
      types(out["insert.sql"]!).params,
      "{ id: number; flag: boolean; big?: number | bigint | string | null; amount?: string | null; label?: string | null; title?: string | null; raw?: Uint8Array | null; uid?: string | null; day?: string | null; at?: string | null; stamp?: string | null; zoned?: string | null }",
    );
    const q = await built(root, "insert", insert);
    const exact = await exactDriver(db);
    const values = {
      id: 3, flag: true, big: 9223372036854775807n, amount: "-99999999.9999", label: "x".repeat(20), title: "😀".repeat(20), raw: new Uint8Array([1, 2, 3]), uid: "00000000-0000-0000-0000-000000000001",
      day: "9999-12-31", at: "00:00:00.001", stamp: "2026-01-02T03:04:05.1234567", zoned: "2026-01-02T03:04:05.123-07:00",
    };
    assert.deepEqual(plain(await q.run(db.pool, values)), { id: 3 });
    const back = await built(root, "back", "select big, label, title, raw, uid, day, at, stamp from t where id = :id");
    assert.deepEqual(plain(await back.run(db.pool, { id: 3 })), { big: 9223372036854775807n, label: values.label, title: values.title, raw: { bytes: [1, 2, 3] }, uid: "00000000-0000-0000-0000-000000000001", day: "9999-12-31", at: "00:00:00.001", stamp: "2026-01-02T03:04:05.1234567" });
    // A datetimeoffset keeps the offset it was given on any tedious, read here as numbers and text.
    const offset = await built(root, "offset", "select datepart(tz, zoned) as tz, cast(cast(zoned as datetime2(3)) as nvarchar(30)) as local from t where id = :id");
    assert.deepEqual(plain(await offset.run(db.pool, { id: 3 })), { tz: -420, local: "2026-01-02 03:04:05.123" });
    if (exact) {
      const decimals = await built(root, "decimals", "select amount, zoned from t where id = :id");
      assert.deepEqual(plain(await decimals.run(db.pool, { id: 3 })), { amount: "-99999999.9999", zoned: "2026-01-02T03:04:05.123-07:00" });
    }
    // A decimal a double holds is bound on any tedious; one it doesn't, only where tedious writes it exactly.
    const wide = await built(root, "wide", "select cast(cast(:d as decimal(38, 10)) as nvarchar(50)) as d");
    assert.deepEqual(await wide.run(db.pool, { d: "12.5" }), { d: "12.5000000000" });
    if (exact) assert.deepEqual(await wide.run(db.pool, { d: "1234567890123456789.0123456789" }), { d: "1234567890123456789.0123456789" });
    else await assert.rejects(wide.run(db.pool, { d: "1234567890123456789.0123456789" }), /Parameter "d" is a SQL Server decimal, which tedious would write as 1234567890123456800: Reylo binds its exact text, which tedious writes with the pool's options\.decimalAsString set to true\./);
    // A text or binary value longer than its column is the server's to meet: no match, or error 2628 on a write
    // (a binary's trailing zeros are cut without one, as a text's trailing spaces are).
    const byLabel = await built(root, "by_label", "select id from t where label = :label");
    assert.deepEqual(plain(await byLabel.run(db.pool, { label: values.label })), [{ id: 3 }]);
    assert.deepEqual(await byLabel.run(db.pool, { label: values.label + "y" }), []);
    const byCode = await built(root, "by_code", "select id from t where code = :code");
    assert.deepEqual(await byCode.run(db.pool, { code: "abcd" }), []);
    await assert.rejects(q.run(db.pool, { ...values, id: 5, label: values.label + "y" }), (e: any) => e.number === 2628);
    await assert.rejects(q.run(db.pool, { ...values, id: 6, title: values.title + "😀" }), (e: any) => e.number === 2628);
    await assert.rejects(q.run(db.pool, { ...values, id: 7, raw: new Uint8Array(9).fill(1) }), (e: any) => e.number === 2628);
    // A time's text that is none is refused before anything is sent.
    await assert.rejects(q.run(db.pool, { ...values, id: 4, day: "tomorrow" }), /Parameter "day" is a SQL Server date, and "tomorrow" is not its text/);
  } finally {
    await db.drop();
  }
});

test("SQL Server: a statement the server refuses is a diagnostic at its place, and .try() reads a failed run as a QueryError with its number and constraint", { skip: sqlserverLive, timeout: 120_000 }, async () => {
  const db = await sqlserverDatabase("reylo_sqlserver_errors", SCHEMA, { decimalAsString: true });
  try {
    const root = project(db.url);
    const out = await transform(root, { "missing.sql": "select nope from t", "batch.sql": "declare @x int = 1; select @x as x", "exec.sql": "exec sp_who" });
    assert.match(out["missing.sql"]!.diagnostics[0]!.messageText, /Invalid column name 'nope'/);
    assert.match(out["batch.sql"]!.diagnostics[0]!.messageText, /one statement/);
    assert.match(out["exec.sql"]!.diagnostics[0]!.messageText, /procedure/i);
    const insert = await built(root, "dupe", "insert into t (id, flag) values (:id, 0)");
    await assert.rejects(insert.run(db.pool, { id: 1 }), (e: any) => e.name === "RequestError" && e.number === 2627);
    const { error } = await insert.try(db.pool, { id: 1 });
    assert.equal(error?.name, "QueryError");
    assert.equal(error.number, 2627);
    assert.equal(error.table, "dbo.t");
    assert.match(error.constraint, /^PK__t__/);
    assert.match(error.message, /^Violation of PRIMARY KEY constraint 'PK__t__\w+'\. Cannot insert duplicate key in object 'dbo\.t'\. The duplicate key value is \(1\)\.$/);
  } finally {
    await db.drop();
  }
});
