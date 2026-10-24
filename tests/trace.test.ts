import { test } from "node:test";
import assert from "node:assert/strict";
import { AsyncLocalStorage } from "node:async_hooks";
import { tracingChannel } from "node:diagnostics_channel";
import { DatabaseSync } from "node:sqlite";
import { defineQuery, type QueryTrace } from "../src/index.ts";
import { listening } from "../src/trace.ts";

// Every query Reylo runs is published on the tracing channel "reylo:query" (src/trace.ts): one
// call per `.run()`, `.runSync()`, `.try()`, `.trySync()` and call of the function `.with()`
// returns, with the query, the client, the parameters, whether it is synchronous, and its result
// or error.
const channel = tracingChannel<QueryTrace>("reylo:query");

const getUser = defineQuery<{ id: number }, { id: number; email: string }, "one", "sqlite">({ dialect: "sqlite", sql: "select id, email from users where id = :id", params: ["id"], cardinality: "one", source: "queries/get_user.sql" });
const createUser = defineQuery<{ email: string }, never, "none", "sqlite">({ dialect: "sqlite", sql: "insert into users (email) values (:email)", params: ["email"], cardinality: "none", source: "queries/create_user.sql" });

function database() {
  const db = new DatabaseSync(":memory:");
  db.exec("create table users (id integer primary key, email text not null unique); insert into users (email) values ('ada@example.com')");
  return db;
}

type Event = { name: string; ctx: QueryTrace; result?: unknown; error?: unknown };

/** Runs `fn` with a subscriber to every event, and returns the events with the result and error each saw. */
async function traced(fn: () => unknown): Promise<Event[]> {
  const events: Event[] = [];
  const at = (name: string) => (ctx: QueryTrace) => events.push({ name, ctx, result: ctx.result, error: ctx.error });
  const subscribers = { start: at("start"), end: at("end"), asyncStart: at("asyncStart"), asyncEnd: at("asyncEnd"), error: at("error") };
  channel.subscribe(subscribers);
  try {
    await fn();
  } finally {
    channel.unsubscribe(subscribers);
  }
  return events;
}

const names = (events: Event[]) => events.map((e) => e.name);

test(".run() publishes one call, which ends when its promise settles", async () => {
  const db = database();
  const events = await traced(async () => assert.deepEqual(await getUser.run(db, { id: 1 }), { id: 1, email: "ada@example.com" }));
  assert.deepEqual(names(events), ["start", "end", "asyncStart", "asyncEnd"]);
  const { ctx } = events[0]!;
  assert.ok(events.every((e) => e.ctx === ctx), "one context from start to asyncEnd");
  assert.equal(ctx.query, getUser);
  assert.equal(ctx.query.source, "queries/get_user.sql");
  assert.equal(ctx.client, db);
  assert.deepEqual(ctx.params, { id: 1 });
  assert.equal(ctx.sync, false);
  assert.equal(events[1]!.result, undefined, "end comes before the promise settles");
  assert.deepEqual(events[3]!.result, { id: 1, email: "ada@example.com" });
});

test(".runSync() publishes one synchronous call, which ends on end", async () => {
  const db = database();
  const events = await traced(() => getUser.runSync(db, { id: 1 }));
  assert.deepEqual(names(events), ["start", "end"]);
  assert.equal(events[0]!.ctx.sync, true);
  assert.deepEqual(events[1]!.result, { id: 1, email: "ada@example.com" });
});

test("a failed call publishes its error: the driver's own, or Reylo's about the row count", async () => {
  const db = database();
  const rowCount = await traced(() => getUser.run(db, { id: 2 }).catch(() => {}));
  assert.deepEqual(names(rowCount), ["start", "end", "error", "asyncStart", "asyncEnd"]);
  assert.match((rowCount[2]!.error as Error).message, /^queries\/get_user\.sql: Expected exactly one row, got 0$/);

  const unique = await traced(() => assert.throws(() => createUser.runSync(db, { email: "ada@example.com" }), /UNIQUE constraint failed/));
  assert.deepEqual(names(unique), ["start", "error", "end"]);
  assert.match((unique[1]!.error as Error).message, /UNIQUE constraint failed: users\.email/);
});

test(".try() and .trySync() publish the database's error they return as a value, once each", async () => {
  const db = database();
  let result: unknown;
  const async = await traced(async () => (result = await createUser.try(db, { email: "ada@example.com" })));
  assert.equal((result as { error?: { code: string } }).error?.code, "SQLITE_CONSTRAINT_UNIQUE");
  assert.deepEqual(names(async), ["start", "end", "error", "asyncStart", "asyncEnd"]);

  const sync = await traced(() => (result = createUser.trySync(db, { email: "ada@example.com" })));
  assert.equal((result as { error?: { code: string } }).error?.code, "SQLITE_CONSTRAINT_UNIQUE");
  assert.deepEqual(names(sync), ["start", "error", "end"]);
  assert.equal(sync[0]!.ctx.sync, true);
});

test(".with() publishes nothing itself, and each call of its function publishes one", async () => {
  const db = database();
  let get: ReturnType<typeof getUser.with> | undefined;
  assert.deepEqual(await traced(() => (get = getUser.with(db))), []);
  const events = await traced(async () => {
    await get!({ id: 1 });
    await get!({ id: 1 });
  });
  assert.deepEqual(names(events).filter((n) => n === "start").length, 2);
  assert.deepEqual(events[0]!.ctx.params, { id: 1 });
});

test("a call without parameters publishes them as {}", async () => {
  const count = defineQuery<{}, { n: number }, "one", "sqlite">({ dialect: "sqlite", sql: "select count(*) as n from users", params: [], cardinality: "one" });
  const events = await traced(() => count.run(database()));
  assert.deepEqual(events[0]!.ctx.params, {});
  assert.equal(events[0]!.ctx.query.source, undefined);
});

test("a store bound to start is the current one while the driver runs, so its spans nest under the query's", async () => {
  const store = new AsyncLocalStorage<string>();
  // The same channel, by its name, with a store of the subscriber's own type.
  const spans = tracingChannel<QueryTrace, string>("reylo:query");
  spans.start.bindStore(store, (ctx) => `span for ${ctx.query.source}`);
  try {
    const db = database();
    const seen: (string | undefined)[] = [];
    // The app's database, whose prepare() notes the current store, as a driver's own tracing would.
    const watched = new Proxy(db, {
      get(target, key) {
        const value = Reflect.get(target, key);
        if (typeof value !== "function") return value;
        return key === "prepare" ? (sql: string) => (seen.push(store.getStore()), target.prepare(sql)) : value.bind(target);
      },
    });
    await getUser.run(watched, { id: 1 });
    getUser.runSync(new Proxy(watched, {}), { id: 1 });
    assert.deepEqual(seen, ["span for queries/get_user.sql", "span for queries/get_user.sql"]);
    assert.equal(store.getStore(), undefined);
  } finally {
    spans.start.unbindStore(store);
  }
});

test("a query builds no context while nothing subscribes, and is published once anything subscribes to any event", () => {
  assert.equal(listening(), undefined);
  for (const event of ["start", "end", "asyncStart", "asyncEnd", "error"] as const) {
    const listener = () => {};
    channel[event].subscribe(listener);
    assert.ok(listening(), event);
    channel[event].unsubscribe(listener);
  }
  assert.equal(listening(), undefined);
});
