/** The dialects of the page's tabs, in their order: every dialect choice on the page is one. */
export const DIALECTS = ['Postgres', 'MySQL', 'SQLite'] as const;
export type DialectName = (typeof DIALECTS)[number];
