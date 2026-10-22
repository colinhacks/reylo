import { runInDurableObject } from "cloudflare:test";
import { env, exports } from "cloudflare:workers";
import { expect, test } from "vitest";
import createPost from "../queries/create_post.sql";
import getUser from "../queries/get_user.sql";
import listPosts from "../queries/list_posts.sql";

test("get_user resolves to a row, or null", async () => {
  const ada = await getUser.run(env.DB, { id: 1 });
  expect(ada).toMatchObject({ id: 1, name: "ada", email: "ada@example.com", role: "admin" });
  expect(await getUser.run(env.DB, { id: 99 })).toBeNull();
});

test("create_post resolves to the row it wrote, and list_posts lists it", async () => {
  const post = await createPost.run(env.DB, { user_id: 1, title: "From a test", body: null });
  expect(post).toEqual({ id: 3, title: "From a test" });
  const posts = await listPosts.run(env.DB, { role: "admin" });
  expect(posts.map((p) => p.title)).toEqual(["Hello", "Again", "From a test"]);
});

test("the Worker answers through its fetch handler", async () => {
  const res = await exports.default.fetch("http://example.com/users/2");
  expect(res.status).toBe(200);
  expect(await res.json()).toMatchObject({ id: 2, name: "bob", email: null });
  expect((await exports.default.fetch("http://example.com/users/99")).status).toBe(404);
});

test("a board, a Durable Object, runs the same queries on a database of its own", async () => {
  const board = env.BOARD.get(env.BOARD.idFromName("test"));
  expect(await board.user(1)).toMatchObject({ id: 1, name: "ada", email: "ada@example.com", role: "admin" });
  expect(await board.post(2, "On the board", null)).toEqual({ id: 3, title: "On the board" });
  // The board's database is its own: D1 has no third post.
  expect((await listPosts.run(env.DB, { role: "member" })).map((p) => p.title)).toEqual([]);
  const res = await exports.default.fetch("http://example.com/boards/test/users/2");
  expect(await res.json()).toMatchObject({ id: 2, name: "bob", email: null });
});

test("inside a board, runSync works in transactionSync, and a throw rolls its writes back", async () => {
  const board = env.BOARD.get(env.BOARD.idFromName("rollback"));
  await runInDurableObject(board, (_board, ctx) => {
    const sql = ctx.storage.sql;
    expect(() =>
      ctx.storage.transactionSync(() => {
        createPost.runSync(sql, { user_id: 1, title: "Kept?", body: null });
        throw new Error("undo");
      }),
    ).toThrow("undo");
    expect(listPosts.runSync(sql, { role: "admin" }).map((p) => p.title)).toEqual(["Hello", "Again"]);
    ctx.storage.transactionSync(() => createPost.runSync(sql, { user_id: 1, title: "Kept", body: "yes" }));
    expect(getUser.runSync(sql, { id: 1 })).toMatchObject({ name: "ada" });
    expect(listPosts.runSync(sql, { role: "admin" }).map((p) => p.title)).toEqual(["Hello", "Again", "Kept"]);
  });
});
