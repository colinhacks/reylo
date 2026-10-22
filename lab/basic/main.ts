import query, * as q from "./query.sql";
import type { Row, Params } from "./query.sql";

declare const pool: { query(text: string, values?: unknown[]): Promise<{ rows: unknown[] }> };

// Exact-type checks: assignable both ways means the inferred type equals the expected one.
type ExpectedRow = {
  id: number;
  name: string;
  email: string | null;
  role: "admin" | "member";
  tags: (string | null)[];
  created_at: string;
  big: bigint | null;
  next_id: number;
};
type ExpectedParams = { id: number; name: string };
const a: ExpectedRow = null! as Row;
const b: Row = null! as ExpectedRow;
const c: ExpectedParams = null! as Params;
const d: Params = null! as ExpectedParams;
const e: Row | null = await query.run(pool, { id: 1, name: "ada" });
// `with` binds a client, such as a transaction, and returns a function of the parameters.
const get = query.with(pool);
const e2: Row | null = await get({ id: 1, name: "ada" });
// @ts-expect-error the function still checks the parameters
await get({ id: 1 });
const f: readonly ["id", "name"] = q.params;
// The parameters are always an object: an array is refused, in the types as at run time.
// @ts-expect-error an array of the values in the order of `params`
await query.run(pool, [1, "ada"]);
// @ts-expect-error the module declares no tuple of the values
type V = q.Values;
const g: ExpectedRow | null = query["~standard"].types!.output;
// As JSON: a BigInt is the integer it is (src/schema.ts).
const h: { created_at: string; big: number | null } | null = query["~reylo"].json;
// @ts-expect-error wrong parameter type
await query.run(pool, { id: "1", name: "ada" });
// @ts-expect-error a sqlite client cannot run a postgres query
await query.run({ prepare: (s: string) => ({ all: () => [] }) }, { id: 1, name: "ada" });

// The engine's inference, without directives: cardinality from the primary key, an aggregate, a
// left join; nested JSON shapes from the constructors; expression nullability from the rules.
import nested from "./nested.sql";
import count from "./count.sql";
import joined from "./joined.sql";
type ExpectedNested = {
  id: number;
  name: string;
  posts: { id: number; title: string; body: string | null; at: string | null }[];
  row: { id: number; name: string; email: string | null; role: "admin" | "member"; tags: (string | null)[]; created_at: string; big: number | null };
  post_count: bigint;
};
const n1: ExpectedNested | null = null! as Awaited<ReturnType<typeof nested.run>>;
const n2: Awaited<ReturnType<typeof nested.run>> = null! as ExpectedNested | null;
const c1: { n: bigint; latest: string | null } = null! as Awaited<ReturnType<typeof count.run>>;
const c2: Awaited<ReturnType<typeof count.run>> = null! as { n: bigint; latest: string | null };
const j1: { id: number; title: string; author: string | null; email: string | null }[] = null! as Awaited<ReturnType<typeof joined.run>>;
const j2: Awaited<ReturnType<typeof joined.run>> = null! as { id: number; title: string; author: string | null; email: string | null }[];
const cardinalities: ["maybeOne", "maybeOne", "one", "many"] = [query.cardinality, nested.cardinality, count.cardinality, joined.cardinality];
// A GROUP BY on the pinned relation's columns is one group at most, whatever is joined; an
// ARRAY constructor is never null.
import grouped from "./grouped.sql";
const g1: { id: number; name: string; posts: bigint; ids: number[] } | null = null! as Awaited<ReturnType<typeof grouped.run>>;
const g2: Awaited<ReturnType<typeof grouped.run>> = null! as { id: number; name: string; posts: bigint; ids: number[] } | null;
const g3: "maybeOne" = grouped.cardinality;
