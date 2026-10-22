// A snapshot of what reylo's Postgres analysis produces for one statement form per file.
// Every type below is what the engine emits (the file type-checks green), and a
// `// WRONG: expected <X>` would mark a place where that is still weaker than the database's
// truth; there is none left.
// Both directions of each assignment are pinned, so the check is exact, and
// `Awaited<ReturnType<typeof q.run>>` pins the cardinality with the row type.

// ---------------------------------------------------------------------------
// 1. json_agg / jsonb_agg of a jsonb_build_object in a correlated subquery
// ---------------------------------------------------------------------------
import jsonAgg from "./json_agg.sql";
import type { Params as JsonAggParams } from "./json_agg.sql";
type JsonAggRow = {
  id: number;
  posts_json: { id: number; title: string; body: string | null }[] | null;
  posts_jsonb: { id: number; views: number }[] | null;
  posts_coalesced: { id: number; at: string | null }[];
};
const jsonAgg1: JsonAggRow | null = null! as Awaited<ReturnType<typeof jsonAgg.run>>;
const jsonAgg2: Awaited<ReturnType<typeof jsonAgg.run>> = null! as JsonAggRow | null;
const jsonAgg3: { id: number } = null! as JsonAggParams;
const jsonAgg4: JsonAggParams = null! as { id: number };
// Right in every part: the shape comes out of the constructors, the field nullability out of
// the catalog, the bare aggregate is nullable because an empty set aggregates to NULL, and
// the coalesce guard removes that null. Cardinality maybeOne from the primary key.

// ---------------------------------------------------------------------------
// 2. array(select ...) and array_agg, of scalars and of row(...)
// ---------------------------------------------------------------------------
import arrays from "./arrays.sql";
type ArraysRow = {
  id: number;
  // An ARRAY subquery is never NULL: an empty subquery is `{}`. Its elements may be NULL, as any
  // array's may, where the engine does not show otherwise.
  post_ids: (number | null)[];
  post_titles: (string | null)[];
  // array_agg over no rows is NULL.
  agg_ids: (number | null)[] | null;
  // `record[]` has no driver parser, so the value arrives as the text Postgres prints,
  // `{"(1,hello)","(2,second)"}`.
  agg_rows: string | null;
};
const arrays1: ArraysRow | null = null! as Awaited<ReturnType<typeof arrays.run>>;
const arrays2: Awaited<ReturnType<typeof arrays.run>> = null! as ArraysRow | null;

// ---------------------------------------------------------------------------
// 3. row(...) constructors, a composite-typed column, to_jsonb of a whole row
// ---------------------------------------------------------------------------
import rows from "./rows.sql";
type RowsRow = {
  // A row value is unparsed text on the wire (`(1,ada)`), and a row constructor is never NULL,
  // cast to a composite type or not.
  pair_anon: string;
  pair_typed: string;
  // Right: to_jsonb(t) gets the whole row's shape, with the JSON twin of every scalar
  // (numeric and int8 as numbers, bytea as text, timestamptz as text).
  whole_json: {
    id: number;
    uid: string;
    name: string;
    email: string | null;
    slug: string;
    role: "admin" | "member";
    tags: (string | null)[];
    meta: unknown;
    score: number | null;
    big: number;
    blob: string | null;
    created_at: string;
  };
  // A whole-row reference arrives as the row's text form; `u` is not on the nullable side of a
  // join, so its row is never NULL.
  whole_row: string;
};
const rows1: RowsRow | null = null! as Awaited<ReturnType<typeof rows.run>>;
const rows2: Awaited<ReturnType<typeof rows.run>> = null! as RowsRow | null;

// ---------------------------------------------------------------------------
// 4. Aggregates, ungrouped and grouped, with FILTER and a window function
// ---------------------------------------------------------------------------
import aggPlain from "./agg_plain.sql";
type AggPlainRow = {
  n: bigint;
  total: bigint | null;
  mean: string | null;
  latest: string | null;
  titles: string | null;
  all_viewed: boolean | null;
  published: bigint;
};
const aggPlain1: AggPlainRow = null! as Awaited<ReturnType<typeof aggPlain.run>>;
const aggPlain2: Awaited<ReturnType<typeof aggPlain.run>> = null! as AggPlainRow;
// A statement without parameters takes no second argument.
declare const client: { query(text: string, values?: unknown[]): Promise<{ rows: unknown[] }> };
const aggPlain3: Promise<AggPlainRow> = aggPlain.run(client);
// Right throughout: an ungrouped aggregate is exactly one row (`one`), count and
// `count(*) filter (...)` are never NULL, every other aggregate is NULL over an empty set,
// sum(int4) is int8, a BigInt, and avg(int4) is numeric, its text (src/values.ts).

import aggGrouped from "./agg_grouped.sql";
type AggGroupedRow = {
  user_id: number;
  n: bigint;
  // A GROUP BY group is never empty and `views` is NOT NULL, so this sum cannot be NULL.
  total: bigint;
  viewed: bigint;
  groups: bigint;
};
const aggGrouped1: AggGroupedRow[] = null! as Awaited<ReturnType<typeof aggGrouped.run>>;
const aggGrouped2: Awaited<ReturnType<typeof aggGrouped.run>> = null! as AggGroupedRow[];
// Right otherwise: `many` for a GROUP BY that no unique key pins, and `count(*) over ()` is a
// non-null int8.

// ---------------------------------------------------------------------------
// 5. WITH: plain, joined, recursive, data-modifying
// ---------------------------------------------------------------------------
import ctePlain from "./cte_plain.sql";
type CtePlainRow = {
  id: number;
  title: string;
  body: string | null;
  // The CTE's own `where published_at is not null` proves it.
  published_at: string;
};
const ctePlain1: CtePlainRow[] = null! as Awaited<ReturnType<typeof ctePlain.run>>;
const ctePlain2: Awaited<ReturnType<typeof ctePlain.run>> = null! as CtePlainRow[];
// Right otherwise: the column types and the catalog NOT NULL survive the CTE.

import cteJoin from "./cte_join.sql";
type CteJoinRow = { id: number; name: string; title: string };
const cteJoin1: CteJoinRow[] = null! as Awaited<ReturnType<typeof cteJoin.run>>;
const cteJoin2: Awaited<ReturnType<typeof cteJoin.run>> = null! as CteJoinRow[];
// Right: `many`, because the unique key pins the user but not the joined CTE rows.

import cteRecursive from "./cte_recursive.sql";
// `select 1 union all select n + 1 from counter where n < 5` is never NULL: the recursive
// query's columns are settled as a fixed point of its own arms.
type CteRecursiveRow = { n: number };
const cteRecursive1: CteRecursiveRow[] = null! as Awaited<ReturnType<typeof cteRecursive.run>>;
const cteRecursive2: Awaited<ReturnType<typeof cteRecursive.run>> = null! as CteRecursiveRow[];

import cteDml from "./cte_dml.sql";
import type { Params as CteDmlParams } from "./cte_dml.sql";
type CteDmlRow = { id: number; title: string; views: number };
// `one`: the WITH query inserts exactly one VALUES row and the outer select reads it with no
// WHERE, so the statement returns that row, like the bare `insert ... returning` of param_insert.
const cteDml1: CteDmlRow = null! as Awaited<ReturnType<typeof cteDml.run>>;
const cteDml2: Awaited<ReturnType<typeof cteDml.run>> = null! as CteDmlRow;
const cteDml3: { uid: number; title: string } = null! as CteDmlParams;
const cteDml4: CteDmlParams = null! as { uid: number; title: string };

// ---------------------------------------------------------------------------
// 6. DECLARE: a cursor statement and a DO block
// ---------------------------------------------------------------------------
// Neither is a statement reylo types, and the engine says so on the statement (reylo10, which
// says that a typed file holds one SELECT, INSERT, UPDATE, DELETE or MERGE statement and how to
// leave a migration out of the type check) rather than handing back `unknown` in silence. A mapper error stops the type check, so the two files live in
// `untyped/`, a project of their own whose check must fail with exactly those two errors.

// ---------------------------------------------------------------------------
// 7. Function calls
// ---------------------------------------------------------------------------
import fnScalar from "./fn_scalar.sql";
type FnScalarRow = {
  greeting: string | null;
  strict_greeting: string | null;
  doubled: number | null;
  email_or: string;
  not_ada: string | null;
  // now() and gen_random_uuid() never return NULL.
  ts: string;
  fresh: string;
  lowered: string | null;
};
const fnScalar1: FnScalarRow | null = null! as Awaited<ReturnType<typeof fnScalar.run>>;
const fnScalar2: Awaited<ReturnType<typeof fnScalar.run>> = null! as FnScalarRow | null;
// Right: a user-defined function's result is nullable (nothing in the catalog says otherwise,
// and a STRICT function with a nullable argument really can return NULL), coalesce with a
// non-null constant is non-null, nullif and lower(nullable) are nullable.

import fnSetof from "./fn_setof.sql";
// Every column is nullable, and that is the honest reading: the function returns SETOF the
// table's row TYPE, and a row type enforces no NOT NULL (the body may select NULL into any
// field). A describe reports table = 0 for a function's columns for the same reason.
type FnSetofRow = {
  id: number | null;
  user_id: number | null;
  title: string | null;
  body: string | null;
  published_at: string | null;
  views: number | null;
};
const fnSetof1: FnSetofRow[] = null! as Awaited<ReturnType<typeof fnSetof.run>>;
const fnSetof2: Awaited<ReturnType<typeof fnSetof.run>> = null! as FnSetofRow[];

import fnTable from "./fn_table.sql";
type FnTableRow = { user_id: number | null; n: bigint | null };
const fnTable1: FnTableRow[] = null! as Awaited<ReturnType<typeof fnTable.run>>;
const fnTable2: Awaited<ReturnType<typeof fnTable.run>> = null! as FnTableRow[];
// Right: `returns table(user_id int, n bigint)` declares no NOT NULL, so nullable is the
// truth, and int/bigint map to number/bigint.

import fnSrf from "./fn_srf.sql";
type FnSrfRow = { id: number; k: unknown };
const fnSrf1: FnSrfRow[] = null! as Awaited<ReturnType<typeof fnSrf.run>>;
const fnSrf2: Awaited<ReturnType<typeof fnSrf.run>> = null! as FnSrfRow[];
// Right, and the best reason string in the lab: a set-returning function in the select list
// makes the row count `many` even though the WHERE pins the primary key
// ("the set-returning function jsonb_path_query returns a row per value, and none for an
// empty set"). `jsonb` is `unknown`, which is the model's own answer for JSON.

// ---------------------------------------------------------------------------
// 8. The column types, plain
// ---------------------------------------------------------------------------
import columns from "./columns.sql";
type ColumnsRow = {
  id: number;
  uid: string;
  name: string;
  email: string | null;
  slug: string;
  role: "admin" | "member";
  tags: (string | null)[];
  meta: unknown;
  score: string | null;
  big: bigint;
  blob: Uint8Array | null;
  created_at: string;
};
const columns1: ColumnsRow[] = null! as Awaited<ReturnType<typeof columns.run>>;
const columns2: Awaited<ReturnType<typeof columns.run>> = null! as ColumnsRow[];
// Right throughout: the enum becomes its label union, the domain over text becomes the base
// type with the column's NOT NULL, uuid, timestamptz and numeric are text and int8 a BigInt
// (src/values.ts), bytea is Uint8Array, text[] is (string | null)[], jsonb is unknown.

// ---------------------------------------------------------------------------
// 9. Nullability through joins
// ---------------------------------------------------------------------------
import joinLeft from "./join_left.sql";
type JoinLeftRow = { id: number; name: string; post_id: number | null; title: string | null; views: number | null };
const joinLeft1: JoinLeftRow[] = null! as Awaited<ReturnType<typeof joinLeft.run>>;
const joinLeft2: Awaited<ReturnType<typeof joinLeft.run>> = null! as JoinLeftRow[];

import joinRight from "./join_right.sql";
type JoinRightRow = { id: number | null; name: string | null; post_id: number; title: string; views: number };
const joinRight1: JoinRightRow[] = null! as Awaited<ReturnType<typeof joinRight.run>>;
const joinRight2: Awaited<ReturnType<typeof joinRight.run>> = null! as JoinRightRow[];

import joinFull from "./join_full.sql";
type JoinFullRow = { id: number | null; name: string | null; post_id: number | null; title: string | null; views: number | null };
const joinFull1: JoinFullRow[] = null! as Awaited<ReturnType<typeof joinFull.run>>;
const joinFull2: Awaited<ReturnType<typeof joinFull.run>> = null! as JoinFullRow[];
// Right: LEFT makes the right side nullable, RIGHT flips it, FULL makes both sides nullable.

import joinNotNull from "./join_notnull.sql";
type JoinNotNullRow = {
  id: number;
  name: string;
  // `where p.id is not null` on the null-supplying side of a LEFT JOIN proves the row matched,
  // so every p column keeps its catalog nullability.
  post_id: number;
  title: string;
  views: number;
};
const joinNotNull1: JoinNotNullRow[] = null! as Awaited<ReturnType<typeof joinNotNull.run>>;
const joinNotNull2: Awaited<ReturnType<typeof joinNotNull.run>> = null! as JoinNotNullRow[];

// ---------------------------------------------------------------------------
// 10. Parameters in every position
// ---------------------------------------------------------------------------
import paramWhere from "./param_where.sql";
import type { Params as ParamWhereParams } from "./param_where.sql";
const paramWhere1: { id: number; name: string } | null = null! as Awaited<ReturnType<typeof paramWhere.run>>;
const paramWhere2: Awaited<ReturnType<typeof paramWhere.run>> = null! as { id: number; name: string } | null;
const paramWhere3: { id: number } = null! as ParamWhereParams;
const paramWhere4: ParamWhereParams = null! as { id: number };

import paramAny from "./param_any.sql";
import type { Params as ParamAnyParams } from "./param_any.sql";
const paramAny1: { id: number; name: string }[] = null! as Awaited<ReturnType<typeof paramAny.run>>;
const paramAny2: Awaited<ReturnType<typeof paramAny.run>> = null! as { id: number; name: string }[];
const paramAny3: { names: string[] } = null! as ParamAnyParams;
const paramAny4: ParamAnyParams = null! as { names: string[] };
// Right: `= any(:names)` types the parameter as the array of the column's type, whose elements
// take no NULL, which would match nothing.

import paramLimit from "./param_limit.sql";
import type { Params as ParamLimitParams } from "./param_limit.sql";
const paramLimit1: { id: number; name: string }[] = null! as Awaited<ReturnType<typeof paramLimit.run>>;
const paramLimit2: Awaited<ReturnType<typeof paramLimit.run>> = null! as { id: number; name: string }[];
// Postgres types a LIMIT placeholder as int8. The `bigint` representation is about how a driver
// DECODES a column; on the way in every driver takes a number, a bigint or a string.
const paramLimit3: { n: number | bigint | string } = null! as ParamLimitParams;
const paramLimit4: ParamLimitParams = null! as { n: number | bigint | string };

import paramInsert from "./param_insert.sql";
import type { Params as ParamInsertParams } from "./param_insert.sql";
const paramInsert1: { id: number; name: string; slug: string } = null! as Awaited<ReturnType<typeof paramInsert.run>>;
const paramInsert2: Awaited<ReturnType<typeof paramInsert.run>> = null! as { id: number; name: string; slug: string };
const paramInsert3: { a: string; b: string } = null! as ParamInsertParams;
const paramInsert4: ParamInsertParams = null! as { a: string; b: string };
// Right: a one-row INSERT ... RETURNING is `one`, the RETURNING columns keep the table's NOT
// NULL, and the domain-typed parameter :b is the base type.

import paramExpr from "./param_expr.sql";
import type { Params as ParamExprParams } from "./param_expr.sql";
type ParamExprRow = {
  // The parameter's own type is `number`, which cannot be null, so the cast of it cannot be.
  x: number;
  y: string;
};
const paramExpr1: ParamExprRow = null! as Awaited<ReturnType<typeof paramExpr.run>>;
const paramExpr2: Awaited<ReturnType<typeof paramExpr.run>> = null! as ParamExprRow;
const paramExpr3: { x: number; y: string | null } = null! as ParamExprParams;
const paramExpr4: ParamExprParams = null! as { x: number; y: string | null };
// Right otherwise: `:x::int` is a number, `coalesce(:y, 'd')` types :y as text the caller may
// pass as null and the column as non-null, and a FROM-less select is exactly one row.

import paramNull from "./param_null.sql";
import type { Params as ParamNullParams } from "./param_null.sql";
type ParamNullParamsType = { email: string | null; name: string; role: "admin" | "member" | null };
const paramNull1: { id: number; email: string | null }[] = null! as Awaited<ReturnType<typeof paramNull.run>>;
const paramNull2: Awaited<ReturnType<typeof paramNull.run>> = null! as { id: number; email: string | null }[];
const paramNull3: ParamNullParamsType = null! as ParamNullParams;
const paramNull4: ParamNullParams = null! as ParamNullParamsType;
// Right: a parameter may be null where the statement lets it be, and nowhere else.
// :email is written whole and bare to a nullable column; :role is
// coalesced, through the WITH that selects it once; :name fills a NOT NULL column. A test for null
// (`input.role is null or …`) would not make :role nullable.

import paramPositional from "./param_positional.sql";
import type { Params as ParamPositionalParams } from "./param_positional.sql";
const paramPositional1: { b: string; a: number } = null! as Awaited<ReturnType<typeof paramPositional.run>>;
const paramPositional2: Awaited<ReturnType<typeof paramPositional.run>> = null! as { b: string; a: number };
const paramPositional3: { 1: number; 2: string } = null! as ParamPositionalParams;
const paramPositional4: ParamPositionalParams = null! as { 1: number; 2: string };
// Right: a positional parameter's key is its number, and each binds by number whatever order the
// statement names them in: $2 is the text, $1 the integer.

// ---------------------------------------------------------------------------
// 11. Views
// ---------------------------------------------------------------------------
import views from "./views.sql";
type ViewsRow = {
  // pg_attribute carries no NOT NULL for a view's columns. The engine reads the view's own
  // SELECT: id and name come from NOT NULL columns, email does not, count() is never NULL, and
  // max() over a LEFT JOIN's far side can be.
  id: number;
  name: string;
  email: string | null;
  posts: bigint;
  last_at: string | null;
};
const views1: ViewsRow[] = null! as Awaited<ReturnType<typeof views.run>>;
const views2: Awaited<ReturnType<typeof views.run>> = null! as ViewsRow[];

// ---------------------------------------------------------------------------
// 12. Casts
// ---------------------------------------------------------------------------
import casts from "./casts.sql";
import type { Params as CastsParams } from "./casts.sql";
type CastsRow = {
  // The server types a cast; its nullability is its argument's, for a built-in target no user
  // cast function reaches.
  id_text: string;
  short_name: string;
  score_int: number | null;
  big_num: string;
  day: string;
  since: string;
  k: number | null;
  nothing: number | null;
};
const casts1: CastsRow | null = null! as Awaited<ReturnType<typeof casts.run>>;
const casts2: Awaited<ReturnType<typeof casts.run>> = null! as CastsRow | null;
// A cast types its parameter: `:since::timestamptz` takes the text that names its zone, as run returns it.
const casts3: { since: string; id: number } = null! as CastsParams;
const casts4: CastsParams = null! as { since: string; id: number };

// ---------------------------------------------------------------------------
// 13. Types no driver parses
// ---------------------------------------------------------------------------
import types from "./types.sql";
type TypesRow = {
  id: number;
  // Ranges, network addresses, money, bit strings, xml, tsvector and composites all arrive as
  // the text Postgres prints. An enum array is parsed, and its elements are the labels.
  span: string;
  during: string | null;
  addr: string;
  price: string | null;
  bits: string | null;
  doc: string | null;
  search: string | null;
  pair: string | null;
  roles: ("admin" | "member" | null)[];
};
const types1: TypesRow | null = null! as Awaited<ReturnType<typeof types.run>>;
const types2: Awaited<ReturnType<typeof types.run>> = null! as TypesRow | null;
