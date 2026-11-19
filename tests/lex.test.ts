import { test } from "node:test";
import assert from "node:assert/strict";
import { lexPlaceholders } from "../src/lex.ts";
import { defineQuery } from "../src/index.ts";
import { engine as load, type Dialect } from "../src/engine.ts";
import { pgPool } from "./stand-ins.ts";

const names = (sql: string, dialect: "postgres" | "mysql" | "sqlite" = "postgres") => lexPlaceholders(sql, dialect).params;

test("the named forms, the dialect's own positional form, and a position is its number", () => {
  assert.deepEqual(names("select :a, @b, $c, :a, arr[lo:hi]"), ["a"]);
  assert.deepEqual(names("select $1, $2, $1, j ? 'k'"), ["1", "2"]);
  assert.deepEqual(names("select ?, ?", "mysql"), ["1", "2"]);
  assert.deepEqual(names("select @x, $1 from t", "mysql"), []);
  // SQLite's own `@name` and `$name` name parameters there, and the name drops the prefix.
  assert.deepEqual(names("select @a.b, a@c, @d, @a.b", "sqlite"), ["a.b", "d"]);
  assert.deepEqual(names("select $a, a$b", "sqlite"), ["a"]);
  // A bare `?` is one past the largest number before it, as SQLite numbers it.
  assert.deepEqual(names("select ?5, ?, ?2", "sqlite"), ["2", "5", "6"]);
  // Positional parameters bind by number, whatever order the statement names them in.
  assert.deepEqual(names("select $2::text, $1::int"), ["1", "2"]);
});

test("a colon that opens a Postgres subscript is a slice, not a parameter", () => {
  // No expression starts with a colon inside a subscript, so `tags[:n]` is `tags[1:n]` there,
  // with whitespace or a comment before the colon, after a name, a quoted name, a parenthesis, a
  // subscript or a placeholder.
  assert.deepEqual(names("select arr[:hi], arr[ :hi], arr[/* c */:hi], \"a\"[:hi], (f(x))[:pg_t.tgnargs], m[1][:hi], arr[1:hi], :a[:hi] from t where x = :x"), ["a", "x"]);
  // A parameter as the subscript is parenthesized; a slice's bound after its colon is a parameter.
  assert.deepEqual(names("select arr[(:i)], arr[ : :hi], arr[1 : :b], arr[:i + 0] from t"), ["i", "hi", "b"]);
  // An array constructor's bracket follows `array` or another bracket, and holds parameters.
  assert.deepEqual(names("select array[:a, :b], ARRAY[[:c], [:d]], x = any(array[:e]) from t"), ["a", "b", "c", "d", "e"]);
  // MySQL has no slices: a colon after no name character opens a parameter there. SQLite reads
  // `[:lo]` as a quoted name, the alias of `arr`.
  assert.deepEqual(names("select arr[:lo] from t", "mysql"), ["lo"]);
  assert.deepEqual(names("select arr[:lo] from t", "sqlite"), []);
});

test("MySQL text is read as a session in the default sql_mode reads it", () => {
  // A backslash escapes the character after it, in either quote: a placeholder after `\'` is
  // still in the string, and one after `'\\'` is not.
  assert.deepEqual(names("select 'it\\'s :not', 'a\\\\', :x, \"say \\\":no\", 'a\\nb' from t where id = :id", "mysql"), ["x", "id"]);
  // Postgres and SQLite read the backslash as a character.
  assert.deepEqual(names("select 'C:\\', :x"), ["x"]);
  assert.deepEqual(names("select 'C:\\', :x", "sqlite"), ["x"]);
  assert.deepEqual(names("select 'C:\\', :x", "mysql"), []);
  // `#` starts a line comment; `--` does only before a space or a control character, or at the end.
  assert.deepEqual(names("select :a # :b\n, :c", "mysql"), ["a", "c"]);
  assert.deepEqual(names("select :a --:b", "mysql"), ["a", "b"]);
  assert.deepEqual(names("select :a -- :b\n, :c --\t:d\n, 1--", "mysql"), ["a", "c"]);
  assert.deepEqual(names("select :a --:b"), ["a"]);
  // A block comment ends at its first `*/`, and backquotes quote a name that escapes nothing.
  assert.deepEqual(names("select /* a /* b */ :x", "mysql"), ["x"]);
  assert.deepEqual(names("select `a\\`, `:b`, :x", "mysql"), ["x"]);
});

test("MySQL text is read as a session in another sql_mode reads it, given that mode", () => {
  const params = (sql: string, backslashEscapes: boolean, ansiQuotes: boolean) => lexPlaceholders(sql, "mysql", { backslashEscapes, ansiQuotes }).params;
  // Under NO_BACKSLASH_ESCAPES a backslash is a character, so the literal ends at its next quote.
  assert.deepEqual(params("select 'C:\\', :x, \"a\\\", :y", false, false), ["x", "y"]);
  assert.deepEqual(params("select 'it''s :not', :x", false, false), ["x"]);
  // Under ANSI_QUOTES a double-quoted word is a name, which escapes nothing.
  assert.deepEqual(params("select \"a\\\", :x", true, true), ["x"]);
  assert.deepEqual(params("select 'it\\'s :not', \":no\", :x", true, true), ["x"]);
});

test("the runtime's lexer agrees with the engine's", async () => {
  // Every database's engine lexes every dialect: the lexer is the core's.
  const engine = await load("sqlite");
  const statements: [string, Dialect][] = [
    ["select 'it\\'s :not', 'a\\\\', :x, \"say \\\":no\", `n:m`, 'a\\nb' from t where id = :id # :c\n and :d --:e\n -- :f\n /* :g */ :h", "mysql"],
    ["select ?, '?', \"\\\"?\", ? -- ?\n # ?\n", "mysql"],
    ["select 'C:\\', :x, \"q\"\":y\", `:z`, ':w' -- :v\n /* :u */ from t where a = :a.b", "postgres"],
    ["select tags[:n], tags[ :m], (f(x))[:pg_t.tgnargs], \"q\"[:k], array[:a, :b], tags[(:c)], tags[1::int:d] from t where y = :y", "postgres"],
    ["select 'C:\\', ?3, `:w`, ? -- :v\n /* :u */", "sqlite"],
    ["select 'C:\\', :x, \"\\\", :y -- :v\n", "sqlite"],
    ["select [a'b] as x, :p as p, [c'd?] as y, [n:m] from t where z = :z", "sqlite"],
    ["SELECT {id:UInt64}, { since : DateTime64(3, 'UTC') }, {e:Enum8('}' = 1, 'it\\'s' = 2)}, c ? a :b, doc.a.:Int64, m[:k], {x}, {1:UInt8}, {id:UInt64} FROM t", "clickhouse"],
    ["SELECT :id::UInt64, x::String FROM t WHERE n IN :ns::Array(UInt8) AND s = :s::String AND :bare", "clickhouse"],
    ["SELECT c ? a :b::String, c ? 'x' /* or */ :d::String, c ? (d ? 1 :e::UInt8) :f::UInt8, c ? :g::UInt8 : 0, c ? 1 : :h::UInt8, :i::UInt8 FROM t", "clickhouse"],
    ["SELECT 1 ? abs(:n::Int32) : 0 AS v, c ? 1 + :m::Int32 : 0, c ? a AND :o::UInt8 : 0, c ? [:p::UInt8, 2] : [], (c ? 1 : 2) + :q::UInt8, c ? f(a) :g::String, c ? m['k'] -- :x::UInt8\n :h::String, c ? $t$?$t$ :j::String FROM t", "clickhouse"],
    ["SELECT 1 AS a$b$, {p:UInt8} AS q, 2 AS c$b$, {x$y:UInt8}, $tag${no:UInt8}$tag$", "clickhouse"],
    ["SELECT c ? {r:UInt8} :s::UInt8, c ? {t:Decimal(9, 2)} + 1 : 0", "clickhouse"],
    ["SELECT 1 ? a$and :b::UInt8, 1 ? a$b$ :c::UInt8, :d::UInt8 FROM (SELECT 1 AS a$and, 2 AS b, 3 AS a$b$, 4 AS c)", "clickhouse"],
    ["SELECT 'it\\'s {a:UInt8}', `{b:UInt8}\\``, \"{c:UInt8}\\\"\" # {d:UInt8}\n #! {e:UInt8}\n /* {f:UInt8} /* {g:UInt8} */ {h:UInt8} */, $q${i:UInt8}$q$, {k:UInt8} -- {j:UInt8}", "clickhouse"],
  ];
  for (const [sql, dialect] of statements) {
    const ours = lexPlaceholders(sql, dialect);
    // Through the engine's long-lived child, and through its one-shot form (a text of its own, so that neither answers from what the other kept).
    for (const theirs of [await engine.lexAsync(sql, dialect), engine.lex(`${sql}\n`, dialect)]) {
      assert.deepEqual(ours.params, theirs.params, `${dialect}: ${sql}`);
      assert.deepEqual(ours.placeholders, theirs.placeholders, `${dialect}: ${sql}`);
    }
  }
});

// Without the engine's rewritten statement (a query defined by hand) the runtime rewrites it itself.
const capture = () => {
  const calls: { text: string; values?: unknown[] | undefined }[] = [];
  return { calls, client: pgPool((text: string, values: unknown[]) => (calls.push({ text, values }), { rows: [{}] })) };
};

test("run binds positional parameters by number", async () => {
  const { calls, client } = capture();
  const q = defineQuery<{ 1: number; 2: string }, {}, "one", "postgres">({ dialect: "postgres", sql: "select $2::text, $1::int", params: ["1", "2"], cardinality: "one" });
  await q.run(client, { 1: 7, 2: "z" });
  assert.deepEqual(calls, [{ text: "select $2::text, $1::int", values: [7, "z"] }]);
});

test("with binds a client; the parameters may be left out when the statement takes none", async () => {
  const { calls, client } = capture();
  const q = defineQuery<{ id: number }, {}, "one", "postgres">({ dialect: "postgres", sql: "select :id::int", params: ["id"], cardinality: "one" });
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

test("a hand-built query whose params miss a placeholder is refused", async () => {
  const { calls, client } = capture();
  const q = defineQuery<{ id: number }, {}, "one", "postgres">({ dialect: "postgres", sql: "select $1", params: ["id"], cardinality: "one" });
  await assert.rejects(q.run(client, { id: 1 }), /placeholder \$1 is parameter "1"/);
  assert.equal(calls.length, 0);
});
