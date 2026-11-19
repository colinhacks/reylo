import { randomUUID } from "node:crypto";
import { createClient } from "@clickhouse/client";
import createPost from "../queries/create_post.sql";
import getUser from "../queries/get_user.sql";
import listPosts from "../queries/list_posts.sql";

// DATABASE_URL is the URL Reylo reads, which opens with `clickhouse+`; the client takes the rest.
const client = createClient({ url: process.env["DATABASE_URL"]!.replace(/^clickhouse\+/, "") });

const user = await getUser.run(client, { id: 1 });
//    ^? { id: number; name: string; email: string | null; role: "admin" | "member"; created_at: string } | null
if (user) console.log(`${user.name} (${user.role}) joined ${user.created_at}`);

// ClickHouse has no serial column and no RETURNING, so the app makes the id it inserts.
const id = randomUUID();
await createPost.run(client, { id, user_id: 1, title: "From main.ts", body: null });
console.log(`created ${id} From main.ts`);

const posts = await listPosts.run(client, { role: "admin" });
//    ^? { id: string; title: string; body: string | null; author: string }[]
for (const post of posts) console.log(`${post.title} by ${post.author}`);

await client.close();
