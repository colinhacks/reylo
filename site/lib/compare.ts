import results from '@/lib/compare-results.json';

/**
 * The docs' comparison, /compare: which tools and rows it shows. Every mark comes from
 * lib/compare-results.json, which lab/compare writes from real runs of each tool.
 */
export type CompareCell = { state: 'y' | 'p' | 'n'; note?: string };

export const compareResults = results as unknown as { at: string; tools: Record<string, { name: string; version: string; cells: Record<string, CompareCell> }> };

/** The tools beside Reylo, in an order that keeps the longer names from sitting side by side. */
export const compareTools = [
  { id: 'sqlc', name: 'sqlc', sub: 'Go', href: 'https://sqlc.dev' },
  { id: 'pgtyped', name: 'PgTyped', sub: 'TypeScript', href: 'https://pgtyped.dev' },
  { id: 'sqlx', name: 'sqlx', sub: 'Rust', href: 'https://github.com/transact-rs/sqlx' },
  { id: 'typedsql', name: 'TypedSQL', sub: 'Prisma', href: 'https://www.prisma.io/docs/orm/prisma-client/using-raw-sql/typedsql' },
  { id: 'safeql', name: 'SafeQL', sub: 'ESLint', href: 'https://safeql.dev' },
  { id: 'typesql', name: 'TypeSQL', sub: 'TypeScript', href: 'https://github.com/wsporto/typesql' },
  { id: 'postgrestypedsql', name: 'Typed SQL', sub: 'Postgres', href: 'https://github.com/fli/postgres-typed-sql' },
];

/** The rows by group, each with what it asks of a tool, shown when its label is hovered. */
export const compareRows: { group: string; rows: { id: string; label: string; asks?: string }[] }[] = [
  {
    group: 'Setup',
    rows: [
      { id: 'no-codegen', label: 'No codegen', asks: 'Types appear with nothing to generate or commit.' },
      { id: 'from-the-database', label: 'Live database', asks: 'Types come from your running database, not from a copy of its schema.' },
    ],
  },
  {
    group: 'Inference',
    rows: [
      { id: 'outer-joins', label: 'Nullable left joins', asks: 'A column from the joined side of a `left join` is typed as possibly null, and the other side is not.' },
      { id: 'expression-nullability', label: 'Nullable expressions', asks: 'It knows that `count(*)` is never null and that `max(x)` can be.' },
      { id: 'parameter-types', label: 'Parameter types', asks: 'Each parameter is typed from the query, with no annotation.' },
      { id: 'parameter-nullability', label: 'Nullable parameters', asks: 'A parameter written to a nullable column takes `null`, and one compared with `=` does not.' },
      { id: 'cardinality', label: 'Cardinality', asks: 'It works out how many rows a query returns: at most one for a lookup by primary key, exactly one for `count(*)`.' },
      { id: 'json-shapes', label: 'Nested JSON', asks: 'The JSON a query builds, as with `json_agg(json_build_object(…))`, is typed by its shape.' },
      { id: 'json-schema', label: 'JSON Schema types', asks: 'A `jsonb` column is typed by the JSON Schema its `CHECK` holds it to.' },
    ],
  },
  {
    group: 'Databases',
    rows: [
      { id: 'postgres', label: 'Postgres' },
      { id: 'mysql', label: 'MySQL' },
      { id: 'sqlite', label: 'SQLite' },
      { id: 'duckdb', label: 'DuckDB' },
    ],
  },
];

export const compareCell = (tool: string, row: string): CompareCell => compareResults.tools[tool]?.cells[row] ?? { state: 'n' };

/** The page's `<CompareTable />` as Markdown, for /compare.md and /llms.txt. */
export function expandCompare(text: string): string {
  if (!text.includes('<CompareTable />')) return text;
  const word = { y: 'yes', p: 'partly', n: 'no' };
  const head = `| | Reylo | ${compareTools.map((t) => `[${t.id === 'postgrestypedsql' ? 'Postgres Typed SQL' : t.name}](${t.href})`).join(' | ')} |`;
  const rule = `| --- | --- | ${compareTools.map(() => '---').join(' | ')} |`;
  const rows = compareRows.flatMap((g) => g.rows).map((r) => `| ${r.label} | ${word[compareCell('reylo', r.id).state]} | ${compareTools.map((t) => word[compareCell(t.id, r.id).state]).join(' | ')} |`);
  return text.replace('<CompareTable />', [head, rule, ...rows].join('\n'));
}
