// Stand-ins for the Postgres clients `.run()` takes, each with what tells its driver's apart
// (src/decode.ts, `takesQuery`): a test's handler gets the statement and its values, and answers
// as the driver does. A client with none of that is refused before it runs anything.

/**
 * pg's Pool, told by its connect() and its Client: `query` gets each statement's text and values,
 * and what it returns is pg's result, a `{ rows }` whose values Reylo's parsers already read.
 */
export function pgPool<R>(query: (text: string, values: unknown[]) => R) {
  return {
    Client: class {},
    connect: (): Promise<never> => Promise.reject(new Error("a stand-in pool hands out no client")),
    query: async (config: string | { text: string; values?: unknown[] }, values?: unknown[]): Promise<Awaited<R>> => await (typeof config === "string" ? query(config, values ?? []) : query(config.text, config.values ?? [])),
  };
}

/** A row as postgres.js's raw() hands it over: the server's text under each column's name. */
export type TextRow = Record<string, string | null>;

/**
 * postgres.js's sql, told by its typed() beside its unsafe(): `unsafe` gets what the real one is
 * called with and returns the rows, which come back through the query's raw() as postgres.js
 * gives them, each column of the type `oids` names (text where it names none).
 */
export function postgresJsSql(unsafe: (text: string, values: unknown[], options?: object) => TextRow[] | void | Promise<TextRow[] | void>, oids: Record<string, number> = {}) {
  return {
    typed: (value: unknown, type: number) => ({ value, type }),
    unsafe: (text: string, values: unknown[] = [], options?: object) => {
      const raw = async () => {
        const rows = (await unsafe(text, values, options)) ?? [];
        const names = Object.keys(rows[0] ?? {});
        return Object.assign(rows.map((r) => names.map((n) => (r[n] == null ? null : Buffer.from(r[n])))), { columns: names.map((name) => ({ name, type: oids[name] ?? 25 })) });
      };
      return Object.assign(Promise.resolve<unknown[]>([]), { raw });
    },
  };
}

/** Neon's neon(), a function with transaction(): before 1.0 it is called with (text, values), and from 1.0 it has query() too. */
export function neonSql<F extends (...args: any[]) => unknown, M extends object = {}>(call: F, members?: M) {
  return Object.assign(call, { transaction: async (): Promise<unknown[]> => [] }, members);
}
