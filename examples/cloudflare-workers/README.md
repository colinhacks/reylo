# Reylo example: Cloudflare Workers

A Cloudflare Worker whose `fetch` handler runs `.sql` files as typed queries on a D1 database. Vite builds it, with the Cloudflare Vite plugin and `reylo/vite`, and serves it in workerd, the Workers runtime, as `wrangler dev` does; the tests run inside workerd too, through Cloudflare's Vitest plugin. Everything here runs locally, with no Cloudflare account: D1 is the local database wrangler keeps under `.wrangler/`.

## Run it

```sh
(cd ../.. && nub run build)  # once, at the repository root: the built packages in dist/ this example links
nub install                  # or pnpm install (npm refuses link: dependencies)
nub run setup                # wrangler d1 migrations apply DB --local, from scratch: prints "created users and posts"
nub run typecheck            # tsc --noEmit --runExternalCode: prints nothing
nub run test                 # vitest run, inside workerd: Tests 3 passed (3)
nub run dev                  # vite dev: the Worker on http://localhost:5173
nub run build                # the type check, then vite build: dist/reylo_example/index.js (about 110 kB) and its wrangler.json
nub run preview              # vite preview: the built Worker on http://localhost:4173
```

With `dev` or `preview` running (`dev` shown):

```text
$ curl localhost:5173/posts
[{"id":1,"title":"Hello","body":"first post","author":"ada"},{"id":2,"title":"Again","body":null,"author":"ada"}]
$ curl localhost:5173/users/1
{"id":1,"name":"ada","email":"ada@example.com","role":"admin","created_at":"2026-10-21 05:12:50"}
$ curl -w ' %{http_code}\n' localhost:5173/users/99
{"error":"not found"} 404
$ curl -d '{"user_id":2,"title":"From curl"}' localhost:5173/posts
{"id":3,"title":"From curl"}
$ curl 'localhost:5173/posts?role=member'
[{"id":3,"title":"From curl","body":null,"author":"bob"}]
```

Each insert stays in the local database; `nub run setup` starts over. The tests get a database of their own. While `nub run dev` runs, an edit to a `.sql` file is picked up on the next request, with no restart.

Misspell a column in `queries/get_user.sql` and `nub run typecheck` reports it at the file's line and column, and `nub run build` stops there:

```text
queries/get_user.sql(1,12): error reylo6: no such column: nmae
```

To deploy, with an account: `wrangler d1 create reylo-example`, put the `database_id` it prints into `wrangler.jsonc`, run `nub run setup` again (the local file takes the new id's name, so update `.reylo.yaml` too, see Notes), then `nub run build` and `nub run deploy`, which applies `migrations/` to the remote database and uploads `dist/`. Without an account, `nubx wrangler deploy --dry-run` after the build checks everything but the upload.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency (the plugin, and the run time each query module imports, which the Worker's bundle carries); `@reylosql/sqlite` (the engine that analyzes each statement), `typescript` 7.1 beta, Vite, the Cloudflare Vite and Vitest plugins and wrangler are dev dependencies. D1 needs no driver package: the binding `env.DB` is the client, and `.run(env.DB, …)` takes it as it is. The `typecheck` script is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper, and `build` runs it before `vite build`, which strips types without checking them.
- In `vite.config.ts`, `reylo()` sits beside `cloudflare()`. It turns each `.sql` import into a module carrying the analyzed statement, in the dev server and in the build. The Cloudflare plugin makes any `.sql` import in a Worker a text module; `reylo()` resolves a relative `.sql` import first, in either order of the list, so the Worker gets the query.
- The `.reylo.yaml` file sets `direct_url` to the local D1 database's file, which `nub run setup` creates: the type check, the editor and `reylo/vite` read the schema there. The file name is fixed by the binding's `database_id` (see Notes). It is in `.reylo.yaml` rather than in a `.env`, because wrangler and the Cloudflare plugin load `.env` into the Worker's `env` as secrets.
- The `migrations/` directory holds the schema as D1 migrations, which `wrangler d1 migrations apply` runs locally (`setup`) and on the remote database (`deploy`), and the tests apply to their own database.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for the project file, `.reylo.yaml` or `.reylo.yml`. The `types` are Workers' (`@cloudflare/workers-types`), the Vitest plugin's (`cloudflare:test`) and Node's, for the config files and `setup.ts`. The file `src/env.d.ts` declares the binding `DB: D1Database`, the type of `env`, and the Worker's module, the type of `exports` in the tests.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported from `src/worker.ts` with a relative path. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote.
- In `vitest.config.ts`, `reylo()` sits beside `cloudflareTest()`, which runs the tests inside workerd with the bindings of `wrangler.jsonc`; it reads `migrations/` into a binding that `test/apply-migrations.ts` applies before the tests. The file `test/queries.test.ts` runs the three queries on `env.DB` and calls the Worker through `exports.default.fetch`.
- In `wrangler.jsonc`, `rules` lists Wrangler's default text types without `.sql`. Wrangler makes every imported `.sql` file a text module, and the Vitest plugin applies that rule before Vite runs any plugin, so without this line each query in a test is the statement's text and `.run` is undefined. The app's build needs no such line, but the rule is Wrangler's own, so it lives there.
- The `.vscode/` and `.zed/` directories hold the editor settings from [Editors](https://reylo.dev/#editors).

## In your own project

- Install from npm instead of the `link:` dependencies: `npm install reylo` and `npm install -D @reylosql/sqlite typescript@beta vite @cloudflare/vite-plugin @cloudflare/vitest-plugin @cloudflare/workers-types @types/node vitest wrangler`.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- **The local database's file name.** Wrangler keeps a local D1 database at `.wrangler/state/v3/d1/miniflare-D1DatabaseObject/<name>.sqlite`, where `<name>` is derived from the binding's `database_id` (workerd's `idFromName` of the id), so it changes with the id and nothing else. After `wrangler d1 migrations apply DB --local`, it is the one `.sqlite` file in that directory beside `metadata.sqlite`; put its path in `.reylo.yaml`.
- **In CI.** The build reads the schema from the local file, which the same migrations make, so a CI job runs `nub run setup` before `nub run build` and needs no access to the remote database.
- **Node's globals.** The Worker sets no `nodejs_compat` flag: Reylo's run time and D1 need none. A compatibility date from August 2026 on gives a Worker `Buffer` and `process` anyway, which Reylo uses where they exist; on an earlier date without the flag the same answers come back. A Postgres driver over TCP (postgres.js or `pg` through Hyperdrive) or `mysql2` needs `nodejs_compat`; Neon's serverless driver, libSQL's web client and PlanetScale's driver do not.
- **Alias imports.** Import `.sql` files by a relative path. A path through an alias (a tsconfig `paths` entry or a Vite alias) reaches the Cloudflare plugin's text rule before Reylo, which resolves relative paths only.
- **Without Vite.** Wrangler's own bundler takes no plugins, but its custom build does: `"build": { "command": "node build.ts", "watch_dir": ["src", "queries"] }` with `"main": "dist/worker.js"` in `wrangler.jsonc`, and a `build.ts` that runs `esbuild.build` with `plugins: [reylo()]` from `reylo/esbuild`, `bundle: true`, `format: "esm"` and `platform: "neutral"`, as in [the esbuild example](../esbuild). Then `wrangler dev` and `wrangler deploy` run that build first. The tests still go through Vitest and `reylo/vite`, so this example uses Vite for both.
- SQLite infers no union from a `check` constraint, so `role` is a `string` here, where Postgres's enum gives `"admin" | "member"`. D1 returns `created_at` as SQLite's text, `2026-10-21 05:12:50`.
