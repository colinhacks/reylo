// `execute` and `executeSync` (src/execute.ts) on a query module a test writes by hand, which names
// its dialect: it runs with that dialect's half of the run time, as `defineQuery` gives it one. A
// module with Reylo's placeholders and no `text` is compiled first, as a build step compiles one,
// by the engine's lexer (`handWritten`): the run time takes a statement without it as written.
import * as run from "../src/execute.ts";
import { postgres } from "../src/decode.ts";
import { mysql } from "../src/decode-mysql.ts";
import { sqlite } from "../src/decode-sqlite.ts";
import { clickhouse } from "../src/decode-clickhouse.ts";
import type { ClientFor, Dialect, ParamsInput, SyncClientFor } from "../src/execute.ts";
import { handWritten } from "./helpers.ts";

/** A query module as a test writes it: its dialect, and no runtime. */
export type QueryModule<D extends Dialect = Dialect> = Omit<run.QueryModule<D>, "runtime">;

const RUNTIMES: { readonly [D in Dialect]: run.Runtime<D> } = { postgres, mysql, sqlite, clickhouse };

// One module with its runtime per module a test wrote, so that what the run time keeps by module
// (src/execute.ts, `moneyParams`) keeps it as it would for a defined query.
const withRuntime = new WeakMap<object, run.QueryModule>();
function runnable<D extends Dialect>(q: QueryModule<D>): run.QueryModule<D> {
  let m = withRuntime.get(q);
  if (!m) {
    const compiled = q.text && q.order ? q : handWritten(q.sql, q.dialect, q.params);
    withRuntime.set(q, (m = { ...q, text: compiled.text, order: compiled.order, marks: compiled.marks, runtime: RUNTIMES[q.dialect] } as run.QueryModule));
  }
  return m as run.QueryModule<D>;
}

export function execute<D extends Dialect>(client: ClientFor<D>, query: QueryModule<D>, params?: ParamsInput): unknown[] | Promise<unknown[]> {
  return run.execute(client, runnable(query), params);
}

export function executeSync<D extends Dialect>(client: SyncClientFor<D>, query: QueryModule<D>, params?: ParamsInput): unknown[] {
  return run.executeSync(client, runnable(query), params);
}
