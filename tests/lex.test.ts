import { test } from "node:test";
import assert from "node:assert/strict";
import { lexPlaceholders } from "../src/lex.ts";
import { defineQuery } from "../src/index.ts";
import { fileURLToPath } from "node:url";
import { engine as load, type Dialect } from "../src/engine.ts";

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
  // MySQL and SQLite have no slices: a colon after no name character opens a parameter there.
  assert.deepEqual(names("select arr[:lo] from t", "mysql"), ["lo"]);
  assert.deepEqual(names("select arr[:lo] from t", "sqlite"), ["lo"]);
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

test("the runtime's lexer agrees with the engine's", async () => {
  // Every database's engine lexes every dialect: the lexer is the core's.
  const engine = await load("sqlite", fileURLToPath(new URL("..", import.meta.url)));
  const statements: [string, Dialect][] = [
    ["select 'it\\'s :not', 'a\\\\', :x, \"say \\\":no\", `n:m`, 'a\\nb' from t where id = :id # :c\n and :d --:e\n -- :f\n /* :g */ :h", "mysql"],
    ["select ?, '?', \"\\\"?\", ? -- ?\n # ?\n", "mysql"],
    ["select 'C:\\', :x, \"q\"\":y\", `:z`, ':w' -- :v\n /* :u */ from t where a = :a.b", "postgres"],
    ["select tags[:n], tags[ :m], (f(x))[:pg_t.tgnargs], \"q\"[:k], array[:a, :b], tags[(:c)], tags[1::int:d] from t where y = :y", "postgres"],
    ["select 'C:\\', ?3, `:w`, ? -- :v\n /* :u */", "sqlite"],
    ["select 'C:\\', :x, \"\\\", :y -- :v\n", "sqlite"],
  ];
  for (const [sql, dialect] of statements) {
    const ours = lexPlaceholders(sql, dialect);
    const theirs = engine.lex(sql, dialect);
    assert.deepEqual(ours.params, theirs.params, `${dialect}: ${sql}`);
    assert.deepEqual(ours.placeholders, theirs.placeholders, `${dialect}: ${sql}`);
  }
});

// Without the engine's rewritten statement (a query defined by hand) the runtime rewrites it itself.
const capture = () => {
  const calls: { text: string; values?: unknown[] | undefined }[] = [];
  return { calls, client: { async query(text: string, values?: unknown[]) { calls.push({ text, values }); return { rows: [{}] }; } } };
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
