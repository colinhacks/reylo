// What the playground's editor completes, read from the database (`reylo play`) or from the docs'
// own PGlite at build time (site/lib/live/play-data.ts): the schema's tables and their columns with
// their types, the functions a query can call with what they return, and the type names a cast
// takes. Free of Node.
import type { Dialect } from "./engine-core.ts";

export type Column = { name: string; type: string; nullable: boolean };
export type Table = { name: string; columns: Column[] };
export type CatalogFunction = {
  name: string;
  kind: "function" | "aggregate";
  /** Its return type, when every form of it returns the same one, and that is a real type. */
  returns?: string;
  /** It returns rows, so it can stand in a FROM clause. */
  rows: boolean;
};
export type Catalog = { tables: Table[]; functions: CatalogFunction[]; types: string[] };

type Queries = { tables: string; functions?: string; types?: string };

/** Each database's catalog queries: rows of `[table, column, type, nullable]`, `[name, aggregate, returns[], rows]` and `[type]`. */
export const CATALOG_QUERIES: Record<Dialect, Queries> = {
  postgres: {
    // The tables and views of the schemas on the search path, as an unqualified name finds them.
    tables: `select c.relname::text, a.attname::text, format_type(a.atttypid, a.atttypmod), not a.attnotnull
  from pg_class c join pg_namespace n on n.oid = c.relnamespace
  join pg_attribute a on a.attrelid = c.oid and a.attnum > 0 and not a.attisdropped
  where n.nspname = any(current_schemas(false)) and n.nspname <> 'pg_catalog' and c.relkind in ('r', 'v', 'm', 'p', 'f') order by c.relname, a.attnum`,
    // Postgres's own and the schema's, without the ones it keeps for itself (input and output,
    // operator and planner support, aggregates' inner steps).
    functions: `select p.proname::text, bool_or(p.prokind = 'a'), array_agg(distinct format_type(p.prorettype, null)), bool_or(p.proretset)
  from pg_proc p join pg_namespace n on n.oid = p.pronamespace
  where (n.nspname = 'pg_catalog' or n.nspname = any(current_schemas(false))) and p.prokind in ('f', 'a', 'w')
    and p.prorettype not in ('internal'::regtype, 'cstring'::regtype, 'trigger'::regtype, 'event_trigger'::regtype, 'language_handler'::regtype,
      'fdw_handler'::regtype, 'index_am_handler'::regtype, 'table_am_handler'::regtype, 'tsm_handler'::regtype)
    and p.proname !~ '^(pg_|_)'
    and p.proname !~ '(_finalfn|_serialize|_deserialize|_combine|_transfn|_support|_sortsupport|_typanalyze|joinsel|sel|_cmp|_hash|_hash_extended|_in|_out|_recv|_send)$'
    and coalesce(obj_description(p.oid, 'pg_proc'), '') !~* '^(I/O|implementation of|internal|deprecated)'
  group by p.proname order by p.proname`,
    // Postgres's own types and the schema's enums and domains.
    types: `select distinct t.typname::text from pg_type t join pg_namespace n on n.oid = t.typnamespace
  where (n.nspname = 'pg_catalog' or n.nspname = any(current_schemas(false))) and t.typtype in ('b', 'e', 'd', 'r', 'm')
    and t.typname !~ '^(_|pg_|reg)' and t.typcategory not in ('A', 'P', 'X', 'Z', 'C')
    and t.typname not in ('aclitem', 'bpchar', 'cid', 'gtsvector', 'name', 'oid', 'refcursor', 'tid', 'txid_snapshot', 'xid', 'xid8')
  order by 1`,
  },
  mysql: {
    tables: `select table_name, column_name, column_type, is_nullable = 'YES' from information_schema.columns
  where table_schema = database() order by table_name, ordinal_position`,
  },
  sqlite: {
    // Each column named apart: a host may read a row by its columns' names.
    tables: `select m.name as table_name, p.name as column_name, p.type as column_type, not p."notnull" as nullable from sqlite_schema m join pragma_table_info(m.name) p
  where m.type in ('table', 'view') and m.name not like 'sqlite_%' order by m.name, p.cid`,
    functions: `select name, max(type = 'a') as aggregate, null as returns, 0 as set_returning from pragma_function_list where name not like 'sqlite_%' group by name order by name`,
  },
  duckdb: {
    // The tables and views of the current schema, as an unqualified name finds them.
    tables: `select table_name, column_name, data_type, is_nullable from duckdb_columns()
  where database_name = current_database() and schema_name = current_schema() and not internal order by table_name, column_index`,
    functions: `select function_name, bool_or(function_type = 'aggregate'), list(distinct return_type) filter (where return_type is not null), bool_or(function_type in ('table', 'table_macro'))
  from duckdb_functions() where function_type in ('scalar', 'aggregate', 'macro', 'table', 'table_macro') and not starts_with(function_name, '_')
  group by function_name order by function_name`,
    types: `select distinct type_name from duckdb_types() where not starts_with(type_name, '_') order by 1`,
  },
};

/** The type names each database's casts take beyond what its catalog lists: the SQL standard's spellings, or every name where it lists none. */
const SPELLED: Record<Dialect, string[]> = {
  postgres: ["bigint", "boolean", "char", "character", "character varying", "decimal", "double precision", "int", "integer", "real", "smallint", "timestamp with time zone"],
  mysql: ["binary", "char", "date", "datetime", "decimal", "double", "float", "json", "nchar", "real", "signed", "time", "unsigned", "year"],
  sqlite: ["blob", "integer", "numeric", "real", "text"],
  duckdb: ["bigint", "blob", "boolean", "date", "decimal", "double", "float", "hugeint", "integer", "interval", "json", "smallint", "time", "timestamp", "timestamptz", "tinyint", "ubigint", "uhugeint", "uinteger", "usmallint", "utinyint", "uuid", "varchar"],
};

/** A type as the list shows it: Postgres's short names for the long standard ones. */
export function shortType(type: string): string {
  return type
    .replace(/^timestamp with time zone/, "timestamptz")
    .replace(/^timestamp without time zone/, "timestamp")
    .replace(/^time with time zone/, "timetz")
    .replace(/^time without time zone/, "time")
    .replace(/^character varying/, "varchar");
}

/** The catalog from its queries' rows; a database with no query for functions or types gives none, or the names it takes. */
export function catalog(dialect: Dialect, tables: unknown[][], functions: unknown[][] = [], types: unknown[][] = []): Catalog {
  const byName = new Map<string, Table>();
  for (const [table, name, type, nullable] of tables as [string, string, string, unknown][]) {
    let t = byName.get(table);
    if (!t) byName.set(table, (t = { name: table, columns: [] }));
    t.columns.push({ name, type: shortType(String(type ?? "")), nullable: Boolean(Number(nullable) || nullable === true) });
  }
  return {
    tables: [...byName.values()],
    functions: (functions as [string, unknown, string[] | null, unknown][]).map(([name, aggregate, returns, rows]) => {
      // A polymorphic or pseudo return type (anyelement, record, void) says nothing worth showing.
      const real = returns?.length === 1 && !/^(any|record|void|"any")/.test(returns[0]!) ? shortType(returns[0]!) : undefined;
      return { name, kind: Number(aggregate) || aggregate === true ? "aggregate" : "function", ...(real ? { returns: real } : {}), rows: Boolean(Number(rows) || rows === true) };
    }),
    types: [...new Set([...(types as [string][]).map(([t]) => t), ...SPELLED[dialect]])].sort(),
  };
}
