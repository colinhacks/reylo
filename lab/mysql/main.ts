import query, * as q from "./query.sql";
import type { Row, Params } from "./query.sql";
import count, { type Row as CountRow, type Params as CountParams } from "./count.sql";
import insert, { type Params as InsertParams } from "./insert.sql";
import nested, { type Row as NestedRow } from "./nested.sql";

declare const conn: { execute(text: string, values?: unknown): Promise<[unknown, unknown]> };
type ExpectedRow = {
  id: number;
  name: string;
  email: string | null;
  big: bigint;
  n: string;
  at: string;
  j: unknown;
  // A TINYINT(1), which BOOLEAN creates, read straight from its table: 0 or 1, or an error.
  flag: boolean;
  raw: Uint8Array | null;
  next_id: bigint;
};
const a: ExpectedRow = null! as Row;
const b: Row = null! as ExpectedRow;
const c: { id: number; skip: string } = null! as Params;
const d: Params = null! as { id: number; skip: string };
// The primary key is equated, so the engine proves at most one row: no `-- @returns` needed.
const e: Row | null = await query.run(conn, { id: 1, skip: "" });
const f: readonly ["id", "skip"] = q.params;
// @ts-expect-error a postgres pool cannot run a mysql query
await query.run({ query: async () => ({ rows: [] }) }, { id: 1, skip: "" });
// An ungrouped aggregate is exactly one row; `count(*)` is a BIGINT, a BigInt on every driver, and
// a DATETIME its text (src/values.ts); `max` of a NOT NULL column is nullable because the server says so
// (no rows, no maximum).
const g: { n: bigint; latest: string | null } = null! as CountRow;
const h: CountRow = null! as { n: bigint; latest: string | null };
const i: { pattern: string } = null! as CountParams;
const j: { n: bigint; latest: string | null } = await count.run(conn, { pattern: "a%" });
// MySQL has no RETURNING, so a data-modifying statement returns nothing; the `?` in a VALUES
// list is typed by the column it fills, and :email, written whole to a nullable column, may be null
// or left out.
const k: { name: string; email?: string | null } = null! as InsertParams;
const k2: InsertParams = null! as { name: string; email?: string | null };
const l: void = await insert.run(conn, { name: "eve", email: "eve@example.com" });
const l2: void = await insert.run(conn, { name: "eve", email: null });
// JSON constructors are typed from their arguments: the engine prepares `select p.id, p.title, p.tag
// from posts p` and `select u.email, u.flag from users u` and reads the server's types and NOT NULL
// flags back into the shape. The `coalesce(json_arrayagg(...), json_array())` under an ungrouped
// aggregate subquery is never null; `json_object` never is; `tag` is nullable in the table.
// The server flags `u.id` nullable next to a correlated aggregate subquery; the engine prepares
// the statement without the subqueries and takes NOT NULL from there. `count(*)` in an ungrouped
// subquery is one row and never null; `max` is null over no rows, and an INT a temporary table may
// widen to a BIGINT, a BigInt either way. Inside a document MySQL writes the TINYINT(1) as the number.
type ExpectedNested = { id: number; posts: { id: number; title: string; tag: string | null }[]; meta: { email: string | null; flag: number }; n_posts: bigint; last_post: bigint | null };
const m: ExpectedNested = null! as NestedRow;
const n: NestedRow = null! as ExpectedNested;
const o: ExpectedNested | null = await nested.run(conn, { id: 1 });
// A positional parameter's key is its number: MySQL's `?, ?` are 1 and 2.
import positional from "./positional.sql";
const pos1: { name: string } | null = null! as Awaited<ReturnType<typeof positional.run>>;
const pos2: Awaited<ReturnType<typeof positional.run>> = null! as { name: string } | null;
const pos3: { 1: number; 2: string } = null! as Parameters<typeof positional.run>[1];
const pos4: Parameters<typeof positional.run>[1] = null! as { 1: number; 2: string };
// @ts-expect-error positional placeholders take an object keyed by number, never an array
await positional.run(conn, [1, "ada"]);
// A backslash escapes the quote after it and `#` starts a comment, so `:not` and `:nobody` are text.
import escaped, * as esc from "./escaped.sql";
const esc1: { id: number; said: string; path: string } | null = await escaped.run(conn, { id: 1 });
const esc2: readonly ["id"] = esc.params;
