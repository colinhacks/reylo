// Creates the tables of setup.sql, with their rows, in the database DATABASE_URL names.
import { readFileSync } from "node:fs";
import { createClient } from "@clickhouse/client";

const url = process.env["DATABASE_URL"];
if (!url) throw new Error("Set DATABASE_URL, in .env or in the environment");

// DATABASE_URL is the URL Reylo reads, which opens with `clickhouse+`; the client takes the rest.
const client = createClient({ url: url.replace(/^clickhouse\+/, "") });
// ClickHouse's HTTP interface runs one statement a request.
for (const statement of readFileSync("setup.sql", "utf8").split(/;\s*$/m)) {
  if (statement.trim()) await client.command({ query: statement });
}
await client.close();
console.log("created users and posts");
