// What the editor completes, read from the playground's own Postgres at build time
// (lib/live/play-data.ts): the schema's tables and their columns with their types, Postgres's
// built-in functions with what they return, and the type names a cast takes.

export type Column = { name: string; type: string; nullable: boolean };
export type Table = { name: string; columns: Column[] };
export type Function = {
  name: string;
  kind: 'function' | 'aggregate';
  /** Its return type, when every form of it returns the same one, and that is a real type. */
  returns?: string;
  /** It returns rows, so it can stand in a FROM clause. */
  rows: boolean;
};
export type Catalog = { tables: Table[]; functions: Function[]; types: string[] };

/** The schema's tables, views and their columns, in order. */
export const TABLES_QUERY = `select c.relname::text, a.attname::text, format_type(a.atttypid, a.atttypmod), not a.attnotnull
  from pg_class c join pg_namespace n on n.oid = c.relnamespace
  join pg_attribute a on a.attrelid = c.oid and a.attnum > 0 and not a.attisdropped
  where n.nspname = 'public' and c.relkind in ('r', 'v', 'm', 'p') order by c.relname, a.attnum`;

/**
 * The functions a query can call: Postgres's own and the schema's, without the ones it keeps for
 * itself (input and output, operator and planner support, aggregates' inner steps).
 */
export const FUNCTIONS_QUERY = `select p.proname::text, bool_or(p.prokind = 'a'), array_agg(distinct format_type(p.prorettype, null)), bool_or(p.proretset)
  from pg_proc p join pg_namespace n on n.oid = p.pronamespace
  where n.nspname in ('pg_catalog', 'public') and p.prokind in ('f', 'a', 'w')
    and p.prorettype not in ('internal'::regtype, 'cstring'::regtype, 'trigger'::regtype, 'event_trigger'::regtype, 'language_handler'::regtype,
      'fdw_handler'::regtype, 'index_am_handler'::regtype, 'table_am_handler'::regtype, 'tsm_handler'::regtype)
    and p.proname !~ '^(pg_|_)'
    and p.proname !~ '(_finalfn|_serialize|_deserialize|_combine|_transfn|_support|_sortsupport|_typanalyze|joinsel|sel|_cmp|_hash|_hash_extended|_in|_out|_recv|_send)$'
    and coalesce(obj_description(p.oid, 'pg_proc'), '') !~* '^(I/O|implementation of|internal|deprecated)'
  group by p.proname order by p.proname`;

/** The types a cast names: Postgres's own and the schema's enums and domains. */
export const TYPES_QUERY = `select t.typname::text from pg_type t join pg_namespace n on n.oid = t.typnamespace
  where n.nspname in ('pg_catalog', 'public') and t.typtype in ('b', 'e', 'd', 'r', 'm')
    and t.typname !~ '^(_|pg_|reg)' and t.typcategory not in ('A', 'P', 'X', 'Z', 'C')
    and t.typname not in ('aclitem', 'bpchar', 'cid', 'gtsvector', 'name', 'oid', 'refcursor', 'tid', 'txid_snapshot', 'xid', 'xid8')
  order by 1`;

/** The SQL standard's spellings, which Postgres takes as well as its own names. */
const STANDARD_TYPES = ['bigint', 'boolean', 'char', 'character', 'character varying', 'decimal', 'double precision', 'int', 'integer', 'real', 'smallint', 'timestamp with time zone'];

/** A type as the list shows it: Postgres's short names for the long standard ones. */
export function shortType(type: string): string {
  return type
    .replace(/^timestamp with time zone/, 'timestamptz')
    .replace(/^timestamp without time zone/, 'timestamp')
    .replace(/^time with time zone/, 'timetz')
    .replace(/^time without time zone/, 'time')
    .replace(/^character varying/, 'varchar');
}

export function catalog(tables: unknown[][], functions: unknown[][], types: unknown[][]): Catalog {
  const byName = new Map<string, Table>();
  for (const [table, name, type, nullable] of tables as [string, string, string, boolean][]) {
    let t = byName.get(table);
    if (!t) byName.set(table, (t = { name: table, columns: [] }));
    t.columns.push({ name, type: shortType(type), nullable });
  }
  return {
    tables: [...byName.values()],
    functions: (functions as [string, boolean, string[], boolean][]).map(([name, aggregate, returns, rows]) => {
      // A polymorphic or pseudo return type (anyelement, record, void) says nothing worth showing.
      const real = returns.length === 1 && !/^(any|record|void|"any")/.test(returns[0]!) ? shortType(returns[0]!) : undefined;
      return { name, kind: aggregate ? 'aggregate' : 'function', ...(real ? { returns: real } : {}), rows };
    }),
    types: [...new Set([...(types as [string][]).map(([t]) => t), ...STANDARD_TYPES])].sort(),
  };
}
