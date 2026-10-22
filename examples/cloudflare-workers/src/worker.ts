// Each .sql import is a typed query: the type check types it from the database, and reylo/vite
// builds it into a module that runs it on the D1 binding, or on a Durable Object's own database.
import { DurableObject } from "cloudflare:workers";
import migration from "../migrations/0001_users_and_posts.sql?raw";
import createPost from "../queries/create_post.sql";
import getUser from "../queries/get_user.sql";
import listPosts from "../queries/list_posts.sql";

// A board keeps users and posts of its own, in its Durable Object's SQLite database, made by the
// same migration as D1's, so the same queries run on it. `.runSync()` takes `ctx.storage.sql`,
// which answers at once.
export class Board extends DurableObject<Cloudflare.Env> {
  constructor(ctx: DurableObjectState, env: Cloudflare.Env) {
    super(ctx, env);
    const made = ctx.storage.sql.exec("select count(*) as n from sqlite_master where name = 'users'").one().n;
    if (!made) ctx.storage.sql.exec(migration);
  }

  user(id: number) {
    return getUser.runSync(this.ctx.storage.sql, { id });
  }

  post(user_id: number, title: string, body: string | null) {
    return createPost.runSync(this.ctx.storage.sql, { user_id, title, body });
  }
}

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    const userId = /^\/users\/(\d+)$/.exec(url.pathname)?.[1];
    const onBoard = /^\/boards\/([\w-]+)\/users\/(\d+)$/.exec(url.pathname);

    if (request.method === "GET" && url.pathname === "/posts") {
      const role = url.searchParams.get("role") ?? "admin";
      return Response.json(await listPosts.run(env.DB, { role }));
    }
    if (request.method === "GET" && userId) {
      const user = await getUser.run(env.DB, { id: Number(userId) });
      return user ? Response.json(user) : Response.json({ error: "not found" }, { status: 404 });
    }
    if (request.method === "GET" && onBoard) {
      const board = env.BOARD.get(env.BOARD.idFromName(onBoard[1]!));
      const user = await board.user(Number(onBoard[2]));
      return user ? Response.json(user) : Response.json({ error: "not found" }, { status: 404 });
    }
    if (request.method === "POST" && url.pathname === "/posts") {
      const { user_id, title, body = null } = await request.json<{ user_id: number; title: string; body?: string | null }>();
      return Response.json(await createPost.run(env.DB, { user_id, title, body }), { status: 201 });
    }
    return Response.json({ error: "not found" }, { status: 404 });
  },
} satisfies ExportedHandler<Cloudflare.Env>;
