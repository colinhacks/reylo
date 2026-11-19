import { createClient } from "@clickhouse/client";
import query from "./query.sql";
import count from "./count.sql";
import insert from "./insert.sql";
import update from "./update.sql";
import remove from "./delete.sql";
import nested from "./nested.sql";
import cast from "./cast.sql";
import inList from "./in_list.sql";
import escaped from "./escaped.sql";
// DATABASE_URL is the one Reylo reads, `clickhouse+http://…`: the client takes it without `clickhouse+`.
const client = createClient({ url: process.env.DATABASE_URL!.replace(/^clickhouse\+/, "") });
// ClickHouse has no transaction to roll back: eve is inserted, renamed by a lightweight UPDATE and deleted again.
await insert.run(client, { id: 3, name: "eve" });
const after = await count.run(client, { pattern: "%" });
await update.run(client, { id: 3, name: "eva" });
const renamed = { ...(await cast.run(client, { id: 3, pattern: "ev%" })), email: (await query.run(client, { id: 3, skip: "" }))?.email };
await remove.run(client, { id: 3 });
const left = await count.run(client, { pattern: "%" });
// A BigInt, which JSON.stringify refuses, is printed with its `n`.
console.log(JSON.stringify({ dialect: query.dialect, ada: await query.run(client, { id: 1, skip: "" }), nobody: await query.run(client, { id: 99, skip: "" }), count: await count.run(client, { pattern: "a%" }), after, renamed, left, nested: await nested.run(client, { id: 1 }), inList: await inList.run(client, { roles: ["member"] }), escaped: await escaped.run(client, { id: 2 }) }, (_, v) => (typeof v === "bigint" ? `${v}n` : v)));
await client.close();
