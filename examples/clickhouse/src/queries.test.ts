import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { after, test } from "node:test";
import { createClient } from "@clickhouse/client";
import createPost from "../queries/create_post.sql";
import deletePost from "../queries/delete_post.sql";
import getUser from "../queries/get_user.sql";
import listPosts from "../queries/list_posts.sql";

const client = createClient({ url: process.env["DATABASE_URL"]!.replace(/^clickhouse\+/, "") });
after(() => client.close());

test("get_user finds ada, and null for an id nobody has", async () => {
  const ada = await getUser.run(client, { id: 1 });
  assert.equal(ada?.name, "ada");
  assert.equal(ada?.role, "admin");
  // An instant is its ISO 8601 text in UTC, which Temporal.Instant.from reads.
  assert.match(ada?.created_at ?? "", /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?Z$/);
  assert.equal(await getUser.run(client, { id: 999 }), null);
});

test("create_post inserts a row, which list_posts lists and delete_post removes", async () => {
  // ClickHouse has no transaction to roll back, so the test deletes the post it inserts.
  const id = randomUUID();
  await createPost.run(client, { id, user_id: 1, title: "From the test", body: "draft" });
  try {
    const posts = await listPosts.run(client, { role: "admin" });
    assert.deepEqual(posts.map((p) => p.title).slice(0, 2), ["Hello", "Again"]);
    assert.equal(posts[1]?.body, null);
    assert.ok(posts.every((p) => p.author === "ada"));
    assert.equal(posts.at(-1)?.id, id);
  } finally {
    await deletePost.run(client, { id });
  }
  const left = await listPosts.run(client, { role: "admin" });
  assert.ok(left.every((p) => p.id !== id));
});
