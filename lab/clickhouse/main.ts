import query, * as q from "./query.sql";
import type { Row, Params } from "./query.sql";
import count, { type Row as CountRow, type Params as CountParams } from "./count.sql";
import insert, { type Params as InsertParams } from "./insert.sql";
import update, { type Params as UpdateParams } from "./update.sql";
import remove, { type Params as DeleteParams } from "./delete.sql";
import nested, { type Row as NestedRow } from "./nested.sql";

declare const client: { exec(params: unknown): Promise<unknown>; query(params: unknown): Promise<unknown>; insert(params: unknown): Promise<unknown>; command(params: unknown): Promise<unknown> };
type ExpectedRow = {
  id: number;
  name: string;
  email: string | null;
  // A UInt64 is a BigInt, a Decimal its text and a DateTime64 its instant's ISO 8601 text (src/values.ts).
  big: bigint;
  n: string;
  at: string;
  // An Array's type says whether an element can be NULL: a String's cannot.
  tags: string[];
  flag: boolean;
  role: "admin" | "member";
  // A Map's keys can repeat, so it is its pairs.
  attributes: { key: string; value: string }[];
  // ClickHouse widens `UInt32 + 1` to a UInt64.
  next_id: bigint;
};
const a: ExpectedRow = null! as Row;
const b: Row = null! as ExpectedRow;
const c: { id: number; skip: string } = null! as Params;
const d: Params = null! as { id: number; skip: string };
// ClickHouse enforces no unique key, so `LIMIT 1` is what makes this at most one row.
const e: Row | null = await query.run(client, { id: 1, skip: "" });
const f: readonly ["id", "skip"] = q.params;
// @ts-expect-error a mysql2 connection cannot run a clickhouse query
await query.run({ execute: async () => [[], []] }, { id: 1, skip: "" });
// @ts-expect-error a parameter declared a UInt32 takes a number
await query.run(client, { id: "1", skip: "" });
// An aggregate with no GROUP BY is exactly one row. `count()` is a UInt64, and `max` of a column
// that is not Nullable is not either: over no rows ClickHouse gives the type's default, not NULL.
const g: { n: bigint; latest: string } = null! as CountRow;
const h: CountRow = null! as { n: bigint; latest: string };
const i: { pattern: string } = null! as CountParams;
const j: { n: bigint; latest: string } = await count.run(client, { pattern: "a%" });
// ClickHouse has no RETURNING, so a write returns nothing. Each parameter declares its type, and
// one declared Nullable may be null or left out.
const k: { id: number; name: string; email?: string | null } = null! as InsertParams;
const k2: InsertParams = null! as { id: number; name: string; email?: string | null };
const l: void = await insert.run(client, { id: 3, name: "eve", email: "eve@example.com" });
const l2: void = await insert.run(client, { id: 3, name: "eve", email: null });
const l3: void = await insert.run(client, { id: 3, name: "eve" });
// @ts-expect-error a String that is not Nullable takes no null
await insert.run(client, { id: 3, name: null, email: null });
const u: { name: string; id: number } = null! as UpdateParams;
const u2: UpdateParams = null! as { name: string; id: number };
const u3: void = await update.run(client, { id: 3, name: "eva" });
const r: { id: number } = null! as DeleteParams;
const r2: void = await remove.run(client, { id: 3 });
// A Tuple with names is an object and one without a tuple, each element by its own type, and an
// aggregate over them an array. `max` of a UInt32 stays one.
type ExpectedNested = { id: number; posts: { id: number; title: string; tag: string | null }[]; pairs: [number, string][]; n_posts: bigint; last_post: number };
const m: ExpectedNested = null! as NestedRow;
const n: NestedRow = null! as ExpectedNested;
const o: ExpectedNested | null = await nested.run(client, { id: 1 });
// `:name::Type` declares a parameter as `{name:Type}` does.
import cast from "./cast.sql";
const cast1: { name: string } | null = null! as Awaited<ReturnType<typeof cast.run>>;
const cast2: Awaited<ReturnType<typeof cast.run>> = null! as { name: string } | null;
const cast3: { id: number; pattern: string } = null! as Parameters<typeof cast.run>[1];
const cast4: Parameters<typeof cast.run>[1] = null! as { id: number; pattern: string };
// An Array parameter is an array, and an Enum each of its names.
import inList from "./in_list.sql";
const in1: { id: number; name: string }[] = await inList.run(client, { roles: ["admin", "member"] });
const in2: { roles: ("admin" | "member")[] } = null! as Parameters<typeof inList.run>[1];
// @ts-expect-error "owner" is no name of the Enum
await inList.run(client, { roles: ["owner"] });
// A backslash escapes the quote after it and `--` starts a comment, so `:not` and `{nobody:String}`
// are text, and the `:` of a conditional names no parameter.
import escaped, * as esc from "./escaped.sql";
const esc1: { id: number; said: string; era: string } | null = await escaped.run(client, { id: 1 });
const esc2: readonly ["id"] = esc.params;
