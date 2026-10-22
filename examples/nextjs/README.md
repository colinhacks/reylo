# Reylo example: Next.js

A Next.js 16.4 App Router app whose page, server action and route handler run `.sql` files as typed queries against Postgres through postgres.js, built with Turbopack or webpack. It is the setup of [Next.js](https://reylo.dev/#setup-nextjs) in the docs, on the app `create-next-app` generates (TypeScript, App Router, no Tailwind, no ESLint).

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/nextjs
nub install                   # or pnpm install; npm does not take the link: dependencies
createdb reylo_nextjs         # any Postgres database you can drop tables in
cp .env.example .env          # DATABASE_URL=postgres://localhost:5432/reylo_nextjs
nub run setup                 # creates users and posts: "created users and posts"
nub run typecheck             # next typegen, then tsc --noEmit --runExternalCode: "✓ Types generated successfully", exit 0
nub run dev                   # next dev (Turbopack) on http://localhost:3000; nub run dev:webpack for webpack
nub run build                 # next build (Turbopack), whose type check runs Reylo's: "◐ /" and "ƒ /api/users/[id]"
nub run start                 # next start on http://localhost:3000
nub run build:webpack         # the same build with webpack; then nub run start
```

With `dev` or `start` running, `/` lists the posts with a form that adds one, and:

```text
$ curl localhost:3000/api/users/1
{"id":1,"name":"ada","email":"ada@example.com","role":"admin","created_at":"2026-10-20T23:17:58.136786Z"}
$ curl -w ' %{http_code}\n' localhost:3000/api/users/99
{"error":"not found"} 404
```

While `nub run dev` or `nub run dev:webpack` runs, an edit to a `.sql` file is picked up on the next request, with no restart: change `order by p.id` to `order by p.id desc` in `queries/list_posts.sql` and reload `/`.

Misspell a column in `queries/list_posts.sql` and `nub run typecheck` reports it at the file's line and column:

```text
queries/list_posts.sql(1,14): error reylo6: column p.titel does not exist (42703)
```

The loader reports the same error to `next dev` and `next build`, which stops there, as `queries/list_posts.sql:1:14: column p.titel does not exist (42703)`. A query's caller fails the build too: misspell `post.author` in `app/page.tsx` and `nub run build` stops after the compile, with Turbopack or webpack:

```text
app/page.tsx(24,40): error TS2551: Property 'authr' does not exist on type '{ id: number; title: string; body: string | null; author: string; }'. Did you mean 'author'?
⨯ Failed to run runAfterProductionCompile: Failed to type check (tsc --runExternalCode).
```

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency (the loader, and the run time each query module imports); `@reylosql/postgres` (the engine that analyzes each statement) and `typescript` 7.1 beta are dev dependencies. The `postgres` package is the driver the app runs the queries with, and the type check reads the schema through it too.
- In the `package.json` scripts, `typecheck` is `next typegen && tsc --noEmit --runExternalCode`, the type check without a build: `next typegen` writes the route types (`LayoutProps`, `RouteContext`) the app uses, and `--runExternalCode` lets `tsc` run the content mapper. The other scripts are Next's own commands: `dev`, `build`, `start`, and the webpack variants `dev:webpack` and `build:webpack`.
- In `next.config.ts`, `withReylo` from `reylo/next` wraps the config. It adds a `turbopack.rules` entry that sends `*.sql` through `reylo/turbopack`, with `condition: { not: "browser" }` so only server code may import a query, and `as: "*.js"` so Turbopack treats the loader's output as JavaScript, and a `webpack` hook that adds `reylo/webpack`, the same loader under webpack's name, on the server only, for `--webpack`. And it moves the type check: `next build` checks with the project's `tsc` but without `--runExternalCode`, so on its own it fails on every `.sql` import (TS2307, and TS18068 on `tsconfig.json`). The wrapper sets `typescript.ignoreBuildErrors` to turn that check off, and runs `tsc --noEmit --runExternalCode` in `compiler.runAfterProductionCompile`, after the compile and before the prerender; `next build` still prints "Skipping validation of types" for its own check. The option `turbopack.root` is there only because Reylo is linked from this repository (below).
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql` and for the project file, `.reylo.yaml` or `.reylo.yml`; the rest is what `create-next-app` writes. Its `include` takes `.ts` and `.tsx` files only, so `setup.sql`, a script rather than one query, stays out of the type check, and the queries come in through their imports.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported as `@/queries/<name>.sql` through the template's `@/*` path. They sit in a top-level `queries/` beside `lib/` rather than in `app/`, because they are the app's data layer, shared by routes, and no route segment owns them. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote.
- The module `lib/db.ts` holds the one postgres.js client every query runs on, marked `server-only`, and kept on `globalThis` so that `next dev` does not open another pool each time it evaluates the module again.
- The page `app/page.tsx` holds a server component that runs `list_posts.sql`, and a server action, `addPost`, that runs `create_post.sql` and calls `revalidatePath("/")`. The file `app/api/users/[id]/route.ts` holds a route handler that runs `get_user.sql` and answers 404 for `null`. Only server code imports a query: a client component (`"use client"`) that imports one fails the build (below).
- The `.env` file sets `DATABASE_URL`, which Reylo reads for its analysis (the type check, the editor and the loader) and Next.js loads for the app. The `setup` script loads it with `node --env-file-if-exists=.env`. A `DATABASE_URL` in the environment wins over `.env` in each.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## In your own project

- Install from npm instead of the `link:` dependencies: `npm install reylo postgres` and `npm install -D @reylosql/postgres typescript@beta`.
- Drop `turbopack.root` from `next.config.ts`. Turbopack resolves nothing outside its root, and the `link:` dependency's real path is this repository's `dist/`, outside this directory; an app that installs `reylo` keeps the default root.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- **Client components.** A client component that imports a `.sql` file fails the build, so a statement never reaches the browser.
- **The edge runtime.** On the edge runtime, use a driver that connects over HTTP or WebSockets, such as Neon's.
- **Cache Components.** Version 16.4 of `create-next-app` turns on `cacheComponents` (and `partialPrefetching`), and this example keeps them. Under Cache Components, a component that queries the database at request time must sit inside `<Suspense>`, as `Posts` does in `app/page.tsx`: without it, `next build` stops with "Next.js encountered uncached or runtime data during prerendering". The page then prerenders its shell and streams the list (`◐ /` in the build output). To prerender the list instead, mark the component `"use cache"`; the build then queries the database.
- **A query in client code.** A `"use client"` component that imports a `.sql` file fails `next build` and the page in `next dev`: Turbopack says `queries/get_user.sql` is an "Unknown module type", webpack that it needs "an appropriate loader". The type check does not catch it, since types know nothing of the client boundary; keep queries behind server code, as `lib/db.ts`'s `server-only` keeps the client.
- **The server action from curl.** A form whose `action` is a server action works without JavaScript: the page carries a hidden `$ACTION_ID_<id>` field, and a multipart POST of it with the form's fields runs the action. Running `curl -H 'Origin: http://localhost:3000' -F '$ACTION_ID_<id>=' -F user_id=2 -F title=Hi -F body= localhost:3000/`, with the id read from the page, adds a post.
- **The `server-only` package.** Next.js handles `import "server-only"` itself and declares its types, so the package need not be installed.
- **The `AGENTS.md` file** is what `next dev` writes for coding agents (turn it off with `agentRules: false`); it is kept so that running the app leaves no new file.
