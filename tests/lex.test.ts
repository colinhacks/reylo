import { test } from "node:test";
import assert from "node:assert/strict";
import { defineQuery } from "../src/index.ts";
import { engine as load, type Dialect } from "../src/engine.ts";
import { handWritten } from "./helpers.ts";
import { pgPool } from "./stand-ins.ts";

test("the engine's lexer answers alike through its long-lived child and its one-shot form", async () => {
  // Every database's engine lexes every dialect: the lexer is the core's.
  const engine = await load("sqlite");
  const statements: [string, Dialect, string[]][] = [
    ["select 'it\\'s :not', 'a\\\\', :x, \"say \\\":no\", `n:m`, 'a\\nb' from t where id = :id # :c\n and :d --:e\n -- :f\n /* :g */ :h", "mysql", ["x", "id", "d", "e", "h"]],
    ["select ?, '?', \"\\\"?\", ? -- ?\n # ?\n", "mysql", ["1", "2"]],
    ["select 'C:\\', :x, \"q\"\":y\", `:z`, ':w' -- :v\n /* :u */ from t where a = :a.b", "postgres", ["x", "a.b"]],
    ["select tags[:n], tags[ :m], (f(x))[:pg_t.tgnargs], \"q\"[:k], array[:a, :b], tags[(:c)], tags[1::int:d] from t where y = :y", "postgres", ["a", "b", "c", "y"]],
    ["select E'it\\'s :not', $$costs $2 or :more$$, $q$:no$q$ as note, :id::int as id", "postgres", ["id"]],
    ["select 'C:\\', ?3, `:w`, ? -- :v\n /* :u */", "sqlite", ["3", "4"]],
    ["select 'C:\\', :x, \"\\\", :y -- :v\n", "sqlite", ["x", "y"]],
    ["select [a'b] as x, :p as p, [c'd?] as y, [n:m] from t where z = :z", "sqlite", ["p", "z"]],
    ["SELECT {id:UInt64}, { since : DateTime64(3, 'UTC') }, {e:Enum8('}' = 1, 'it\\'s' = 2)}, c ? a :b, doc.a.:Int64, m[:k], {x}, {1:UInt8}, {id:UInt64} FROM t", "clickhouse", ["id", "since", "e"]],
    ["SELECT :id::UInt64, x::String FROM t WHERE n IN :ns::Array(UInt8) AND s = :s::String AND :bare", "clickhouse", ["id", "ns", "s"]],
    ["SELECT c ? a :b::String, c ? 'x' /* or */ :d::String, c ? (d ? 1 :e::UInt8) :f::UInt8, c ? :g::UInt8 : 0, c ? 1 : :h::UInt8, :i::UInt8 FROM t", "clickhouse", ["g", "h", "i"]],
    ["SELECT 1 ? abs(:n::Int32) : 0 AS v, c ? 1 + :m::Int32 : 0, c ? a AND :o::UInt8 : 0, c ? [:p::UInt8, 2] : [], (c ? 1 : 2) + :q::UInt8, c ? f(a) :g::String, c ? m['k'] -- :x::UInt8\n :h::String, c ? $t$?$t$ :j::String FROM t", "clickhouse", ["n", "m", "o", "p", "q"]],
    ["SELECT 1 AS a$b$, {p:UInt8} AS q, 2 AS c$b$, {x$y:UInt8}, $tag${no:UInt8}$tag$", "clickhouse", ["p", "x$y"]],
    ["SELECT c ? {r:UInt8} :s::UInt8, c ? {t:Decimal(9, 2)} + 1 : 0", "clickhouse", ["r", "t"]],
    ["SELECT 1 ? a$and :b::UInt8, 1 ? a$b$ :c::UInt8, :d::UInt8 FROM (SELECT 1 AS a$and, 2 AS b, 3 AS a$b$, 4 AS c)", "clickhouse", ["d"]],
    ["SELECT 'it\\'s {a:UInt8}', `{b:UInt8}\\``, \"{c:UInt8}\\\"\" # {d:UInt8}\n #! {e:UInt8}\n /* {f:UInt8} /* {g:UInt8} */ {h:UInt8} */, $q${i:UInt8}$q$, {k:UInt8} -- {j:UInt8}", "clickhouse", ["k"]],
  ];
  for (const [sql, dialect, params] of statements) {
    // A text of its own for the one-shot form, so that neither answers from what the other kept.
    const [kept, once] = [await engine.lexAsync(sql, dialect), engine.lex(`${sql}\n`, dialect)];
    assert.deepEqual(kept.params, params, `${dialect}: ${sql}`);
    assert.deepEqual(once.params, kept.params, `${dialect}: ${sql}`);
    assert.deepEqual(once.placeholders, kept.placeholders, `${dialect}: ${sql}`);
  }
});

test("a statement written by hand compiles through the engine's lexer, as a build step compiles one", () => {
  // An `E''` string with an escaped quote and a dollar quote holding `$2` and `:more` hold no placeholder.
  const pg = handWritten("select E'it\\'s :not' as s, $$costs $2 or :more$$ as note, :id::int as id, :name as name", "postgres");
  assert.equal(pg.text, "select E'it\\'s :not' as s, $$costs $2 or :more$$ as note, $1::int as id, $2 as name");
  assert.deepEqual([pg.params, pg.order], [["id", "name"], ["id", "name"]]);
  assert.deepEqual(pg.marks.map(([start, end]) => pg.text.slice(start, end)), ["$1", "$2"]);
  // MySQL and SQLite take one `?` per placeholder, a repeated name repeated in the binding order.
  for (const dialect of ["mysql", "sqlite"] as const) {
    const q = handWritten("select :id as id, ':no' as s, :name as name, :id as again", dialect);
    assert.equal(q.text, "select ? as id, ':no' as s, ? as name, ? as again");
    assert.deepEqual([q.params, q.order], [["id", "name"], ["id", "name", "id"]]);
    assert.deepEqual(q.marks.map(([start, end]) => q.text.slice(start, end)), ["?", "?", "?"]);
  }
  // A mark is a JavaScript offset, past a character UTF-8 writes in more than one byte.
  const wide = handWritten("select 'é€😀' as s, :id as id", "mysql");
  assert.equal(wide.text.slice(...wide.marks[0]!), "?");
});

const capture = () => {
  const calls: { text: string; values?: unknown[] | undefined }[] = [];
  return { calls, client: pgPool((text: string, values: unknown[]) => (calls.push({ text, values }), { rows: [{}] })) };
};

test("a query defined by hand is taken as written: its statement in the database's own placeholders, and params its binding order", async () => {
  const { calls, client } = capture();
  const q = defineQuery<{ 1: number; 2: string }, {}, "one", "postgres">({ dialect: "postgres", sql: "select $2::text, $1::int", params: ["1", "2"], cardinality: "one" });
  assert.equal(q.sql, "select $2::text, $1::int");
  await q.run(client, { 1: 7, 2: "z" });
  // Reylo's own placeholders are no placeholders to the run time, which reads none off the text.
  const named = defineQuery<{ id: number }, {}, "one", "postgres">({ dialect: "postgres", sql: "select :id::int", params: ["id"], cardinality: "one" });
  assert.equal(named.sql, "select :id::int");
  await named.run(client, { id: 1 });
  assert.deepEqual(calls, [{ text: "select $2::text, $1::int", values: [7, "z"] }, { text: "select :id::int", values: [1] }]);
});

test("with binds a client; the parameters may be left out when the statement takes none", async () => {
  const { calls, client } = capture();
  const q = defineQuery<{ id: number }, {}, "one", "postgres">({ dialect: "postgres", ...handWritten("select :id::int", "postgres"), cardinality: "one" });
  const get = q.with(client);
  assert.equal(calls.length, 0);
  await get({ id: 1 });
  const none = defineQuery<Record<never, never>, {}, "one", "postgres">({ dialect: "postgres", sql: "select 1", params: [], cardinality: "one" });
  await none.run(client);
  await none.with(client)();
  // @ts-expect-error a statement that takes parameters needs them
  await q.run(client).catch(() => {});
  assert.deepEqual(calls.slice(0, 3), [{ text: "select $1::int", values: [1] }, { text: "select 1", values: [] }, { text: "select 1", values: [] }]);
});
