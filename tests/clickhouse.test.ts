import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { defineQuery } from "../src/index.ts";
import { canonical, parseType, typeName } from "../src/clickhouse-types.ts";
import { CLICKHOUSE, DRIVERS, INDEX, ROOT, clickhouseDatabase, clickhouseLive, declarations, driver, transform } from "./helpers.ts";

// ClickHouse end to end, on the throwaway server REYLO_TEST_CLICKHOUSE names, each test in a
// database of its own: the types the content mapper writes for a statement, read from the server's
// own description of it, and the rows its module's `.run()` returns on `@clickhouse/client` and
// `@clickhouse/client-web` (lab/drivers), one representation per type (src/values.ts), each held
// to the server's own text of the value.

function project(url: string): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-clickhouse-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  mkdirSync(join(root, "node_modules"), { recursive: true });
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

/** How many rows a declaration says the statement returns: its query's cardinality. */
const returns = (text: string): string => /^declare const query: Query<[\s\S]*?, "(one|maybeOne|many|none)", "clickhouse", QueryError>/m.exec(text)?.[1] ?? "";

/** Each diagnostic of a file, as its message and the text it marks. */
const marked = (sql: string, diagnostics: { messageText: string; start: number; length: number }[]) => diagnostics.map((d) => [d.messageText, sql.slice(d.start, d.start + d.length)]);

/** A query written by hand, with no build step: its statement alone. */
const byHand = (sql: string, params: string[] = []) => defineQuery<Record<string, unknown>, Record<string, unknown>, "many", "clickhouse">({ dialect: "clickhouse", sql, params });

/** Every type the run time reads, each with a value that tries its representation: the SQL that makes it, and the value `.run()` gives. */
const VALUES: [name: string, sql: string, value: unknown][] = [
  ["u8", "toUInt8(255)", 255],
  ["i8", "toInt8(-128)", -128],
  ["u16", "toUInt16(65535)", 65535],
  ["i32", "toInt32(-2147483648)", -2147483648],
  ["u32", "toUInt32(4294967295)", 4294967295],
  ["u64", "toUInt64('18446744073709551615')", 18446744073709551615n],
  ["i64", "toInt64('-9223372036854775808')", -9223372036854775808n],
  ["i128", "toInt128('-170141183460469231731687303715884105728')", -170141183460469231731687303715884105728n],
  ["u128", "toUInt128('340282366920938463463374607431768211455')", 340282366920938463463374607431768211455n],
  ["u256", "toUInt256('115792089237316195423570985008687907853269984665640564039457584007913129639935')", 2n ** 256n - 1n],
  ["i256", "toInt256('-5')", -5n],
  ["f64", "toFloat64('0.1') + toFloat64('0.2')", 0.30000000000000004],
  ["f32", "toFloat32('0.1')", Math.fround(0.1)],
  ["f_nan", "nan", NaN],
  ["f_inf", "-inf", -Infinity],
  ["bf16", "CAST(1.5, 'BFloat16')", 1.5],
  ["dec32", "toDecimal32('-1.50', 2)", "-1.5"],
  ["dec64", "toDecimal64('0.05', 4)", "0.05"],
  ["dec128", "toDecimal128('12345678901234567890.1234567890', 10)", "12345678901234567890.123456789"],
  ["dec256", "toDecimal256('-0.000000000000000000000000000001', 30)", "-0.000000000000000000000000000001"],
  ["dec_zero", "toDecimal64('0', 4)", "0"],
  ["flag", "true", true],
  ["str", "'héllo\\0\\'x'", "héllo\0'x"],
  ["fixed", "toFixedString('ab', 4)", "ab\0\0"],
  ["uuid", "toUUID('00112233-4455-6677-8899-aabbccddeeff')", "00112233-4455-6677-8899-aabbccddeeff"],
  ["ip4", "toIPv4('1.2.3.4')", "1.2.3.4"],
  ["ip6", "toIPv6('2001:db8::1')", "2001:db8::1"],
  ["day", "toDate('2149-06-06')", "2149-06-06"],
  ["day32", "toDate32('1900-01-01')", "1900-01-01"],
  ["at", "toDateTime('2026-01-01 21:00:00', 'Asia/Tokyo')", "2026-01-01T12:00:00Z"],
  ["at9", "toDateTime64('2026-01-01 12:34:56.123456789', 9, 'UTC')", "2026-01-01T12:34:56.123456789Z"],
  ["before", "toDateTime64('1969-12-31 23:59:58.5', 3, 'UTC')", "1969-12-31T23:59:58.500Z"],
  ["at0", "toDateTime64('2026-01-01 00:00:00', 0, 'UTC')", "2026-01-01T00:00:00Z"],
  ["role", "CAST('member', 'Enum8(\\'admin\\' = 1, \\'member\\' = -2)')", "member"],
  ["wide_role", "CAST('b', 'Enum16(\\'a\\' = 1, \\'b\\' = 300)')", "b"],
  ["list", "[toUInt16(1), 2]", [1, 2]],
  ["holes", "[1, NULL, 3]", [1, null, 3]],
  ["none", "emptyArrayString()", []],
  ["nested_list", "[[toUInt64(1)], []]", [[1n], []]],
  ["pair", "(toUInt8(1), 'a')", [1, "a"]],
  ["named", "CAST((1, 'a'), 'Tuple(n UInt8, s String)')", { n: 1, s: "a" }],
  ["twice", "map('k', toUInt8(1), 'k', 2)", [{ key: "k", value: 1 }, { key: "k", value: 2 }]],
  ["absent", "CAST(NULL, 'Nullable(UInt8)')", null],
  ["present", "toNullable(toUInt8(5))", 5],
  ["packed", "toLowCardinality('x')", "x"],
  ["packed_null", "CAST(NULL, 'LowCardinality(Nullable(String))')", null],
  ["either_number", "CAST(7, 'Variant(String, UInt8)')", 7],
  ["either_text", "CAST('a', 'Variant(String, UInt8)')", "a"],
  ["either_null", "CAST(NULL, 'Variant(String, UInt8)')", null],
  ["spot", "CAST((1.5, 2.5), 'Point')", [1.5, 2.5]],
  ["summed", "CAST(1, 'SimpleAggregateFunction(sum, UInt64)')", 1n],
];

/** Each parameter type the run time writes: the type, a value of it, and the value it comes back as where that differs. */
const PARAMS: [type: string, name: string, value: unknown, back?: unknown][] = [
  ["UInt8", "u8", 255],
  ["Int64", "i64", -9223372036854775808n],
  ["UInt64", "u64", 18446744073709551615n],
  ["UInt64", "u64_number", 7, 7n],
  ["UInt256", "u256", 2n ** 256n - 1n],
  ["Float64", "f", 0.1],
  ["Float64", "f_nan", NaN],
  ["Float64", "f_inf", -Infinity],
  ["Decimal(38, 10)", "dec", "12345678901234567890.123456789"],
  ["Bool", "flag", true],
  ["String", "s", "a\tb\nc\\N'd\0é\r"],
  ["String", "s_null_text", "\\N"],
  ["Nullable(String)", "s_null", null],
  ["FixedString(4)", "fixed", "ab", "ab\0\0"],
  ["UUID", "uuid", "61f0c404-5cb3-11e7-907b-a6006ad3dba0"],
  ["IPv4", "ip4", "1.2.3.4"],
  ["IPv6", "ip6", "2001:db8::1"],
  ["Enum8('a' = 1, 'it\\'s' = 2)", "label", "it's"],
  ["Date", "day", "2026-01-02"],
  ["Date32", "day32", "1900-01-01"],
  ["DateTime('Asia/Tokyo')", "at", "2026-01-01T12:00:00Z"],
  ["DateTime", "at_offset", "2026-01-01T21:00:00+09:00", "2026-01-01T12:00:00Z"],
  ["DateTime64(9, 'UTC')", "at9", "2026-01-01T12:34:56.123456789Z"],
  ["DateTime64(3)", "before", "1969-12-31T23:59:58.500Z"],
  ["DateTime64(3, 'UTC')", "at_short", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00.000Z"],
  ["Array(String)", "strings", ["it's", "b\tc\\", ""]],
  ["Array(Nullable(UInt8))", "holes", [1, null]],
  ["Array(Array(UInt64))", "nested_list", [[1n], []]],
  ["Array(Decimal(18, 2))", "decimals", ["1.5", "-2.25"]],
  ["Array(DateTime64(3, 'UTC'))", "instants", ["2026-01-01T00:00:00.500Z"]],
  ["Array(DateTime('UTC'))", "seconds", ["2026-01-01T00:00:00Z"]],
  ["Array(Date)", "days", ["2026-01-02"]],
  ["Array(UUID)", "uuids", ["61f0c404-5cb3-11e7-907b-a6006ad3dba0"]],
  ["Array(Bool)", "flags", [true, false]],
  ["Array(Float64)", "floats", [NaN, Infinity, -0]],
  ["Tuple(UInt64, String)", "pair", [1n, "a'b"]],
  ["Tuple(n UInt8, s Nullable(String))", "named", { n: 1, s: null }],
  ["Array(Tuple(UInt64, String))", "pairs", [[1n, "a"], [2n, "b"]]],
  ["Map(String, Array(UInt8))", "twice", [{ key: "k", value: [1, 2] }, { key: "it's", value: [] }]],
  ["LowCardinality(String)", "packed", "x"],
  ["LowCardinality(Nullable(String))", "packed_null", null],
];

/** Rows compared by value, a NaN equal to a NaN and -0 apart from 0, as `deepStrictEqual` compares them. */
const row = (pairs: [string, unknown][]) => Object.fromEntries(pairs);

test("a ClickHouse type's name reads into its parts and writes back on one line", () => {
  for (const name of ["UInt64", "Nullable(String)", "Decimal(18, 4)", "DateTime64(3, 'UTC')", "Enum8('admin' = 1, 'it\\'s' = -2)", "Tuple(n UInt8, s Nullable(String))", "Tuple(`a b` UInt8)", "Tuple()", "Map(String, Array(UInt8))", "Variant(String, UInt8)", "AggregateFunction(quantiles(0.5, 0.9), UInt64)", "JSON(a.b UInt32, max_dynamic_paths = 8, SKIP c.d)"]) {
    assert.equal(typeName(parseType(name)!), name);
  }
  // As a describe writes a tuple with names, a member a line, and as a result's header writes it.
  assert.equal(canonical("Tuple(\n    x Float64,\n    y Tuple(\n        z String))"), "Tuple(x Float64, y Tuple(z String))");
  assert.equal(canonical("Decimal(18,4)"), "Decimal(18, 4)");
  assert.equal(parseType("Array(String"), undefined);
  assert.equal(parseType("UInt64 extra"), undefined);
});

test("ClickHouse: every type comes back as its one representation, on the Node client and the web client", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const sql = `SELECT ${VALUES.map(([name, value]) => `${value} AS ${name}`).join(", ")}`;
  const wanted = row(VALUES.map(([name, , value]) => [name, value]));
  for (const pkg of ["@clickhouse/client", "@clickhouse/client-web"]) {
    const client = driver(pkg).createClient({ url: CLICKHOUSE! });
    try {
      assert.deepEqual(await byHand(sql).run(client, {}), [wanted], pkg);
    } finally {
      await client.close();
    }
  }
});

test("ClickHouse: a value's text is the server's own", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const client = driver("@clickhouse/client").createClient({ url: CLICKHOUSE! });
  try {
    // An address in every form ClickHouse writes one, each beside the server's text of it.
    const addresses = ["::", "::1", "::2", "2001:db8::1", "::ffff:1.2.3.4", "::1.2.3.4", "::1:0", "::0.0.1.0", "::ffff", "64:ff9b::1.2.3.4", "::fffe:1.2.3.4", "2001:db8:0:1:1:1:1:1", "1::", "1:0:0:2:0:0:0:3", "1::2:0:0", "0:0:1::", "1:2:3:4:5:6:7:8", "ff02::1:2"];
    const [ips] = await byHand(`SELECT ${addresses.map((a, i) => `toIPv6('${a}') AS a${i}, toString(toIPv6('${a}')) AS s${i}`).join(", ")}`).run(client, {});
    addresses.forEach((a, i) => assert.equal(ips![`a${i}`], ips![`s${i}`], a));
    // A decimal at each width, and a date and an instant at their ends.
    const numbers = ["toDecimal32('99999.9999', 4)", "toDecimal32('-0.0001', 4)", "toDecimal64('-999999999999.999999', 6)", "toDecimal128('0.10', 2)", "toDecimal256('1' , 0)", "toDecimal64('100', 2)"];
    const [decimals] = await byHand(`SELECT ${numbers.map((n, i) => `${n} AS d${i}, toString(${n}) AS s${i}`).join(", ")}`).run(client, {});
    numbers.forEach((n, i) => assert.equal(decimals![`d${i}`], decimals![`s${i}`], n));
    const [ends] = await byHand("SELECT toDate('1970-01-01') AS d0, toDate32('2299-12-31') AS d1, toDateTime('2106-02-07 06:28:15', 'UTC') AS t0, toDateTime64('1900-01-01 00:00:00', 6, 'UTC') AS t1, toDateTime64('2262-04-11 23:47:16.854775807', 9, 'UTC') AS t2").run(client, {});
    assert.deepEqual(ends, { d0: "1970-01-01", d1: "2299-12-31", t0: "2106-02-07T06:28:15Z", t1: "1900-01-01T00:00:00.000000Z", t2: "2262-04-11T23:47:16.854775807Z" });
  } finally {
    await client.close();
  }
});

test("ClickHouse: each parameter is written as its type's text, and comes back as given", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const client = driver("@clickhouse/client").createClient({ url: CLICKHOUSE! });
  try {
    const sql = `SELECT ${PARAMS.map(([type, name]) => `{${name}:${type}} AS ${name}`).join(", ")}`;
    const given = row(PARAMS.map(([, name, value]) => [name, value]));
    const wanted = row(PARAMS.map(([, name, value, back]) => [name, back === undefined ? value : back]));
    assert.deepEqual(await byHand(sql, PARAMS.map(([, name]) => name)).run(client, given), [wanted]);
    // The other form, a cast after the name, is sent as ClickHouse's own.
    assert.deepEqual(await byHand("SELECT :n::UInt8 + 1 AS next, :s::Nullable(String) AS s", ["n", "s"]).run(client, { n: 1, s: null }), [{ next: 2, s: null }]);
    // A parameter used twice is one value.
    assert.deepEqual(await byHand("SELECT {n:UInt8} + {n:UInt8} AS sum", ["n"]).run(client, { n: 2 }), [{ sum: 4 }]);
  } finally {
    await client.close();
  }
});

test("ClickHouse: a value its parameter's type cannot hold is refused before the statement runs", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const client = driver("@clickhouse/client").createClient({ url: CLICKHOUSE! });
  const refused = async (type: string, value: unknown) => {
    await assert.rejects(byHand(`SELECT {p:${type}} AS p`, ["p"]).run(client, { p: value }), (e: Error) => e.name === "TypeError" && /^Parameter "p"(?: at \S+)? has the ClickHouse type /.test(e.message), `${type} takes ${String(value)}`);
  };
  try {
    // ClickHouse wraps 300 into a UInt8 as 44, drops a decimal's digits past its scale, and reads a date outside its range as another.
    for (const [type, value] of [["UInt8", 300], ["UInt8", -1], ["UInt8", 1.5], ["Int8", "1"], ["Int64", 2n ** 63n], ["Decimal(18, 2)", "1.239"], ["Decimal(4, 2)", "100"], ["Date", "1960-01-01"], ["Date", "2026-02-30"], ["Date32", "2300-01-01"]] as const) await refused(type, value);
    // An instant names its zone, and keeps the digits its type has.
    for (const [type, value] of [["DateTime", "2026-01-01 12:00:00"], ["DateTime", "2026-01-01T12:00:00.5Z"], ["DateTime", "2107-01-01T00:00:00Z"], ["DateTime64(3)", "2026-01-01T12:00:00.1234Z"], ["DateTime64(3)", "2026-01-01T12:00:00 America/New_York"]] as const) await refused(type, value);
    for (const [type, value] of [["FixedString(2)", "abc"], ["Enum8('a' = 1)", "b"], ["String", 1], ["String", null], ["Array(UInt8)", [1, null]], ["Tuple(UInt8, UInt8)", [1]], ["UUID", "nope"], ["Bool", 1], ["Map(String, UInt8)", { k: 1 }]] as const) await refused(type, value);
    await assert.rejects(byHand("SELECT {p:JSON} AS p", ["p"]).run(client, { p: {} }), /Parameter "p" has the ClickHouse type JSON, which Reylo does not write/);
    // The place inside the parameter is named.
    await assert.rejects(byHand("SELECT {p:Array(Tuple(n UInt8, s String))} AS p", ["p"]).run(client, { p: [{ n: 1, s: "a" }, { n: 300, s: "b" }] }), /^TypeError: Parameter "p" at \[1\]\.n has the ClickHouse type UInt8, which holds 0 to 255; got 300\.$/);
  } finally {
    await client.close();
  }
});

test("ClickHouse: an error is the server's, whether the statement fails at once or after its first rows", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const client = driver("@clickhouse/client").createClient({ url: CLICKHOUSE!, log: { level: 127 } });
  try {
    const atOnce = await byHand("SELECT nope FROM system.one").try(client, {});
    assert.equal(atOnce.error?.name, "QueryError");
    assert.deepEqual([atOnce.error?.code, atOnce.error?.number], ["UNKNOWN_IDENTIFIER", 47]);
    assert.match(atOnce.error!.message, /^Unknown expression identifier `nope`/);
    // The server has sent rows by the time the hundred-thousandth fails, and the error ends the result.
    // Its message is the server's text, byte for byte, whatever characters it holds.
    const late = await byHand("SELECT throwIf(number = 100000, 'boom – déjà vu €') AS x FROM numbers(200000) SETTINGS max_block_size = 1000").try(client, {});
    assert.deepEqual([late.error?.code, late.error?.number], ["FUNCTION_THROW_IF_VALUE_IS_NON_ZERO", 395]);
    assert.match(late.error!.message, /^boom – déjà vu €/);
    // Anything that is no client of ClickHouse's is refused by what one is.
    await assert.rejects(byHand("SELECT 1 AS n").run({ query() {} } as never, {}), /a client of @clickhouse\/client or @clickhouse\/client-web/);
  } finally {
    await client.close();
  }
});

const SCHEMA = `CREATE TABLE users (id UInt64, name String, email Nullable(String), age UInt8, balance Decimal(18, 4), created DateTime64(3, 'UTC'), role Enum8('admin' = 1, 'member' = 2), tags Array(String), meta Map(String, UInt32), spot Tuple(x Float64, y Float64), plan LowCardinality(String)) ENGINE = MergeTree ORDER BY id;
CREATE TABLE orders (id UInt64, user_id UInt64, total Decimal(18, 2), note Nullable(String)) ENGINE = MergeTree ORDER BY id;
INSERT INTO users VALUES (1, 'ada', NULL, 36, 1.5, '2026-01-02 03:04:05.678', 'admin', ['a', 'b'], map('logins', 3), (1.5, 2.5), 'pro'), (2, 'bob', 'bob@example.com', 41, 2.25, '2026-01-03 00:00:00', 'member', [], map(), (0, 0), 'free');
INSERT INTO orders VALUES (10, 1, 9.99, 'first')`;

test("ClickHouse: a select is typed from the server's own description, nullability and nested shapes included", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const db = await clickhouseDatabase("reylo_clickhouse_types", SCHEMA);
  try {
    const root = project(db.url);
    const files = {
      "user.sql": "SELECT * FROM users WHERE id = {id:UInt64} LIMIT 1",
      // An unmatched row of a left join is its type's default on ClickHouse, not NULL, and the server says so.
      "joined.sql": "SELECT u.id, o.total, o.note, groupArray((o.id, o.total)) OVER () AS all FROM users u LEFT JOIN orders o ON o.user_id = u.id ORDER BY u.id",
      "counted.sql": "SELECT count() AS users, max(age) AS oldest, maxOrNull(email) AS last_email FROM users WHERE id > {after:UInt64}",
      "names.sql": "SELECT name FROM users WHERE name LIKE {pattern:String} AND created >= {since:DateTime64(3, 'UTC')} AND id IN {ids:Array(UInt64)} LIMIT {count:UInt64}",
      "total.sql": "SELECT sum(balance) FROM users",
      // ClickHouse types an IN of constants a UInt8, and of a column a Nullable(UInt8): a parameter is a constant as the statement runs.
      "found.sql": "SELECT id, id IN {ids:Array(UInt64)} AS found, id IN (7, {other:UInt64}) AS either FROM users ORDER BY id",
      // A format and an aggregate's parameter are constants to ClickHouse, and neither decides a type.
      "shown.sql": "SELECT name, formatDateTime(created, {format:String}) AS shown FROM users WHERE id = {id:UInt64}",
      "middle.sql": "SELECT quantile({level:Float64})(age) AS middle, count() AS n FROM users",
    };
    const out = await declarations(root, files);
    assert.deepEqual(types(out["user.sql"]!), {
      params: "{ id: number | bigint }",
      row: '{ id: bigint; name: string; email: string | null; age: number; balance: string; created: string; role: "admin" | "member"; tags: string[]; meta: { key: string; value: number }[]; spot: { x: number; y: number }; plan: string }',
    });
    assert.equal(returns(out["user.sql"]!), "maybeOne");
    assert.equal(types(out["joined.sql"]!).row, "{ id: bigint; total: string; note: string | null; all: [bigint, string][] }");
    assert.equal(returns(out["joined.sql"]!), "many");
    assert.deepEqual(types(out["counted.sql"]!), { params: "{ after: number | bigint }", row: "{ users: bigint; oldest: number; last_email: string | null }" });
    assert.equal(returns(out["counted.sql"]!), "one");
    assert.equal(types(out["names.sql"]!).params, "{ pattern: string; since: string; ids: (number | bigint)[]; count: number | bigint }");
    assert.equal(types(out["found.sql"]!).row, "{ id: bigint; found: number; either: number }");
    assert.deepEqual(types(out["shown.sql"]!), { params: "{ format: string; id: number | bigint }", row: "{ name: string; shown: string }" });
    assert.equal(types(out["middle.sql"]!).row, "{ middle: number; n: bigint }");
    assert.equal(returns(out["middle.sql"]!), "one");
    // One column with no name of its own is the statement's value.
    assert.match(out["total.sql"]!, /^declare const query: Query<Record<never, never>, string, "one", "clickhouse", QueryError>/m);

    const user = await built(root, "user", files["user.sql"]);
    assert.deepEqual(await user.run(db.client, { id: 1 }), { id: 1n, name: "ada", email: null, age: 36, balance: "1.5", created: "2026-01-02T03:04:05.678Z", role: "admin", tags: ["a", "b"], meta: [{ key: "logins", value: 3 }], spot: { x: 1.5, y: 2.5 }, plan: "pro" });
    assert.equal(await user.run(db.client, { id: 3n }), null);
    const joined = await built(root, "joined", files["joined.sql"]);
    assert.deepEqual(await joined.run(db.client, {}), [
      { id: 1n, total: "9.99", note: "first", all: [[10n, "9.99"], [0n, "0"]] },
      { id: 2n, total: "0", note: null, all: [[10n, "9.99"], [0n, "0"]] },
    ]);
    const counted = await built(root, "counted", files["counted.sql"]);
    assert.deepEqual(await counted.run(db.client, { after: 0 }), { users: 2n, oldest: 41, last_email: "bob@example.com" });
    assert.deepEqual(await counted.run(db.client, { after: 9 }), { users: 0n, oldest: 0, last_email: null });
    const names = await built(root, "names", files["names.sql"]);
    assert.deepEqual(await names.run(db.client, { pattern: "%", since: "2026-01-02T03:04:05.678Z", ids: [1, 2n], count: 5 }), [{ name: "ada" }, { name: "bob" }]);
    assert.equal(await (await built(root, "total", files["total.sql"])).run(db.client, {}), "3.75");
    const found = await built(root, "found", files["found.sql"]);
    assert.deepEqual(await found.run(db.client, { ids: [2n, 5n], other: 1 }), [{ id: 1n, found: 0, either: 1 }, { id: 2n, found: 1, either: 0 }]);
    const shown = await built(root, "shown", files["shown.sql"]);
    assert.deepEqual(await shown.run(db.client, { format: "%Y/%m", id: 1 }), [{ name: "ada", shown: "2026/01" }]);
    assert.deepEqual(await (await built(root, "middle", files["middle.sql"])).run(db.client, { level: 0.5 }), { middle: 38.5, n: 2n });
    // The web client reads the same bytes.
    const web = driver("@clickhouse/client-web").createClient({ url: db.address });
    try {
      assert.deepEqual(await counted.run(web, { after: 1 }), { users: 1n, oldest: 41, last_email: "bob@example.com" });
    } finally {
      await web.close();
    }
  } finally {
    await db.drop();
  }
});

test("ClickHouse: a result whose types are not the ones the type check read is refused", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const db = await clickhouseDatabase("reylo_clickhouse_drift", SCHEMA);
  try {
    const root = project(db.url);
    const joined = await built(root, "joined", "SELECT u.id, o.total FROM users u LEFT JOIN orders o ON o.user_id = u.id ORDER BY u.id");
    assert.deepEqual(await joined.run(db.client, {}), [{ id: 1n, total: "9.99" }, { id: 2n, total: "0" }]);
    // A client whose session nulls an unmatched row gets a Nullable column, which the type check did not read.
    const nulling = db.connect({ clickhouse_settings: { join_use_nulls: 1 } });
    try {
      await assert.rejects(joined.run(nulling, {}), /joined\.sql returned the column "total" as Nullable\(Decimal\(18, 2\)\) where the type check read Decimal\(18, 2\)\. The table changed after the type check, or the session that runs the statement has a setting the type check's session lacks, as join_use_nulls/);
    } finally {
      await nulling.close();
    }
    // The setting written in the statement is read by the type check and the run alike.
    const out = await declarations(root, { "nulled.sql": "SELECT u.id, o.total FROM users u LEFT JOIN orders o ON o.user_id = u.id ORDER BY u.id SETTINGS join_use_nulls = 1" });
    assert.equal(types(out["nulled.sql"]!).row, "{ id: bigint; total: string | null }");
    // A column whose type changed after the build is refused the same way.
    await db.client.command({ query: "ALTER TABLE orders MODIFY COLUMN total Decimal(18, 4)" });
    await assert.rejects(joined.run(db.client, {}), /returned the column "total" as Decimal\(18, 4\) where the type check read Decimal\(18, 2\)/);
  } finally {
    await db.drop();
  }
});

test("ClickHouse: what cannot be typed is an error at its place", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const db = await clickhouseDatabase("reylo_clickhouse_refused", SCHEMA);
  try {
    const root = project(db.url);
    const files = {
      // The scale of a DateTime64 is read from the parameter's value, so the column has no one type.
      "scale.sql": "SELECT toDateTime64(created, {scale:UInt8}) AS t FROM users WHERE id = {id:UInt64}",
      "cast.sql": "SELECT CAST(age, {type:String}) AS a FROM users",
      "name.sql": "SELECT {column:Identifier} FROM users",
      "untyped.sql": "SELECT id FROM users WHERE id = :id",
      "unknown.sql": "SELECT id, nope FROM users",
      "syntax.sql": "SELECT id FROM users WHERE",
      "json.sql": "SELECT CAST('{\"a\": 1}', 'JSON') AS doc",
      "unnamed.sql": "SELECT id, age + 1 FROM users",
      "two.sql": "SELECT 1 AS a; SELECT 2 AS b",
      "ddl.sql": "CREATE TABLE t (id UInt64) ENGINE = MergeTree ORDER BY id",
      "variant.sql": "SELECT {v:Variant(String, UInt8)} AS v",
    };
    const found = await transform(root, files);
    const of = (name: keyof typeof files) => marked(files[name], found[name]!.diagnostics);
    assert.equal(of("scale.sql").length, 1);
    assert.equal(of("scale.sql")[0]![1], "{scale:UInt8}");
    assert.match(of("scale.sql")[0]![0]!, /^ClickHouse reads \{scale:UInt8\} as a constant here, and the result's types change with its value: column 1 \(t\) has the type DateTime64\(1.* with 1 and DateTime64\(2.* with 2\. Write the value in the statement\.$/);
    assert.equal(of("cast.sql")[0]![1], "{type:String}");
    assert.match(of("cast.sql")[0]![0]!, /^ClickHouse reads \{type:String\} as a constant here, and the statement fails with 'a' in its place/);
    assert.match(of("name.sql")[0]![0]!, /^\{column:Identifier\} stands for a name/);
    assert.match(of("untyped.sql")[0]![0]!, /A ClickHouse parameter is written with its type: \{id:Type\}, or :id::Type\.$/);
    assert.equal(of("untyped.sql")[0]![1], ":");
    assert.match(of("unknown.sql")[0]![0]!, /^Unknown expression identifier `nope`/);
    assert.match(of("syntax.sql")[0]![0]!, /^Syntax error/);
    assert.match(of("json.sql")[0]![0]!, /^Column 1 \(doc\) has the type JSON, and Reylo does not read a ClickHouse JSON yet/);
    assert.match(of("unnamed.sql")[0]![0]!, /^Column 2 has no name: the database calls it "plus\(age, 1\)"/);
    assert.match(of("two.sql")[0]![0]!, /^A typed \.sql file holds one statement and this one holds several/);
    assert.match(of("ddl.sql")[0]![0]!, /^Reylo types a ClickHouse SELECT, INSERT, DELETE or UPDATE, and CREATE is none of them/);
    assert.match(of("variant.sql")[0]![0]!, /is a Variant, whose value's text does not say which of its types it is/);
  } finally {
    await db.drop();
  }
});

test("ClickHouse: a write returns no rows, and is checked against its table", { skip: clickhouseLive, timeout: 120_000 }, async () => {
  const db = await clickhouseDatabase("reylo_clickhouse_writes", SCHEMA);
  try {
    const root = project(db.url);
    const files = {
      "add.sql": "INSERT INTO orders (id, user_id, total, note) VALUES ({id:UInt64}, {user:UInt64}, {total:Decimal(18, 2)}, {note:Nullable(String)})",
      "copy.sql": "INSERT INTO orders SELECT id + {offset:UInt64}, user_id, total, note FROM orders",
      "remove.sql": "DELETE FROM orders WHERE id = {id:UInt64}",
      "bad_column.sql": "INSERT INTO orders (id, nope) VALUES ({id:UInt64}, 1)",
      "bad_where.sql": "DELETE FROM orders WHERE nope = {id:UInt64}",
      "from_body.sql": "INSERT INTO orders FORMAT JSONEachRow",
    };
    const found = await transform(root, files);
    for (const name of ["add.sql", "copy.sql", "remove.sql"] as const) assert.deepEqual(found[name]!.diagnostics, [], name);
    assert.equal(types(found["add.sql"]!.text).params, "{ id: number | bigint; user: number | bigint; total: string; note?: string | null }");
    assert.equal(returns(found["add.sql"]!.text), "none");
    assert.match(found["bad_column.sql"]!.diagnostics[0]!.messageText, /^No such column nope in table/);
    assert.match(found["bad_where.sql"]!.diagnostics[0]!.messageText, /^Unknown expression or function identifier `nope`/);
    assert.match(found["from_body.sql"]!.diagnostics[0]!.messageText, /^An INSERT … FORMAT takes its rows from the request's body/);
    const add = await built(root, "add", files["add.sql"]);
    // The module sends its statement with the comment naming its file, which an INSERT's rows must not run into.
    // A parameter that takes NULL may be left out, and is NULL then.
    assert.equal(await add.run(db.client, { id: 11, user: 2, total: "5.25" }), undefined);
    await assert.rejects(add.run(db.client, { id: 12, total: "1" } as never), /Parameter "user" is missing, and the statement takes no null for it/);
    const read = byHand("SELECT id, total, note FROM orders ORDER BY id");
    assert.deepEqual(await read.run(db.client, {}), [{ id: 10n, total: "9.99", note: "first" }, { id: 11n, total: "5.25", note: null }]);
    const remove = await built(root, "remove", files["remove.sql"]);
    await remove.run(db.client, { id: 10 });
    assert.deepEqual(await read.run(db.client, {}), [{ id: 11n, total: "5.25", note: null }]);
  } finally {
    await db.drop();
  }
});
