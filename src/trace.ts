// Each query Reylo runs is published on Node's tracing channel "reylo:query"
// (https://nodejs.org/api/diagnostics_channel.html#class-tracingchannel), where a tracer or a logger
// subscribes. The module is looked up at run time, never imported, so a bundle for a runtime
// without it (a Worker without nodejs_compat, a browser) still builds, and its queries run untraced.
import type { Dialect } from "./execute.ts";
import type { Cardinality, Query } from "./index.ts";

/**
 * What each event of the "reylo:query" tracing channel carries: one object per call, the same from
 * `start` to `asyncEnd`, so a subscriber may keep its span on it.
 */
export type QueryTrace = {
  /** The query: its `source`, `sql` and `dialect` name it in a span. */
  readonly query: Query<unknown, unknown, Cardinality, Dialect>;
  /** The client it runs on, as the call passed it. */
  readonly client: unknown;
  /** The parameters, as the call passed them: `{}` for none. */
  readonly params: Readonly<Record<string, unknown>>;
  /**
   * True for `.runSync()` and `.trySync()`, whose calls end on `end`; false for `.run()`, `.try()`
   * and the function `.with()` returns, whose promises settle on `asyncEnd`.
   */
  readonly sync: boolean;
  /** What the call returns, set before `end` when `sync`, and before `asyncEnd` otherwise. */
  result?: unknown;
  /** What the call threw or rejected with, set before `error`: a database error `.try()` returns as a value is one too. */
  error?: unknown;
};

type Channel = { readonly hasSubscribers: boolean };
type Tracing = {
  readonly start: Channel;
  readonly end: Channel;
  readonly asyncStart: Channel;
  readonly asyncEnd: Channel;
  readonly error: Channel;
  traceSync<T>(fn: () => T, context: QueryTrace): T;
  tracePromise<T>(fn: () => Promise<T>, context: QueryTrace): Promise<T>;
};

const tracing = ((): Tracing | undefined => {
  try {
    const host = (globalThis as { process?: { getBuiltinModule?(id: string): unknown } }).process;
    const dc = host?.getBuiltinModule?.("node:diagnostics_channel") as { tracingChannel?(name: string): Tracing } | undefined;
    return dc?.tracingChannel?.("reylo:query");
  } catch {
    return undefined;
  }
})();

/**
 * The channel, when anything subscribes to it, so that an untraced query builds no context. Bun's
 * TracingChannel has no `hasSubscribers` of its own, so each event's channel is asked.
 */
export function listening(): Tracing | undefined {
  const t = tracing;
  return t && (t.start.hasSubscribers || t.end.hasSubscribers || t.asyncStart.hasSubscribers || t.asyncEnd.hasSubscribers || t.error.hasSubscribers) ? t : undefined;
}
