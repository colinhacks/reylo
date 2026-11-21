import { createClient } from "@clickhouse/client";
// DATABASE_URL is the one Reylo reads, `clickhouse+http://…`: the client takes it without `clickhouse+`.
if (!process.env.DATABASE_URL) throw new Error("Set DATABASE_URL to a throwaway ClickHouse, as clickhouse+http://127.0.0.1:<port>/lab");
const url = new URL(process.env.DATABASE_URL.replace(/^clickhouse\+/, ""));
const database = url.pathname.slice(1) || "default";
// The database is made through a client that names none, since a client is refused one that is missing.
url.pathname = "/";
const server = createClient({ url: url.href });
await server.command({ query: `create database if not exists \`${database}\`` });
await server.close();
const c = createClient({ url: url.href, database });
await c.command({ query: "drop table if exists posts" });
await c.command({ query: "drop table if exists users" });
// A lightweight UPDATE needs the two block columns, which a table has only where it asks for them.
await c.command({ query: `create table users (id UInt32, name String, email Nullable(String), big UInt64 default 9007199254740993, n Decimal(10, 2) default 1.5, at DateTime64(3, 'UTC') default now64(3), tags Array(String), flag Bool default true, role Enum8('admin' = 1, 'member' = 2) default 'member', attributes Map(String, String)) engine = MergeTree order by id settings enable_block_number_column = 1, enable_block_offset_column = 1` });
await c.command({ query: `insert into users (id, name, email, tags, role, attributes) values (1, 'ada', 'ada@example.com', ['a', 'b'], 'admin', {'plan': 'pro'}), (2, 'bob', null, [], 'member', {})` });
await c.command({ query: `create table posts (id UInt32, user_id UInt32, title String, tag Nullable(String)) engine = MergeTree order by id` });
await c.command({ query: `insert into posts (id, user_id, title, tag) values (1, 1, 'hello', 'intro'), (2, 1, 'again', null)` });
await c.close();
