# Reylo example: esbuild

A small HTTP server on `node:http` that esbuild bundles, with `reylo/esbuild`, into one ES module, `dist/server.mjs`, which runs with plain `node`: no preload, no TypeScript and no `node_modules`. Its queries run on Postgres through postgres.js, and the bundle carries the driver, Reylo's run time and each query's analysis. It is the setup of [esbuild](https://reylo.dev/#setup-esbuild) in the docs.

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/esbuild
nub install                   # or pnpm install (npm does not take link: dependencies)
createdb reylo_esbuild        # any Postgres database you can drop tables in
cp .env.example .env          # DATABASE_URL=postgres://localhost:5432/reylo_esbuild
nub run setup                 # creates users and posts: "created users and posts"
nub run typecheck             # tsc --noEmit --runExternalCode: no output, exit 0
nub run dev                   # the sources under reylo/register, restarted on every change: "listening on http://localhost:3000"
nub run build                 # the type check, then esbuild: writes dist/server.mjs (about 166 kB) and its source map
nub run start                 # node dist/server.mjs: "listening on http://localhost:3000"
```

With `dev` or `start` running:

```text
$ curl localhost:3000/posts
[{"id":1,"title":"Hello","body":"first post","author":"ada"},{"id":2,"title":"Again","body":null,"author":"ada"}]
$ curl localhost:3000/users/1
{"id":1,"name":"ada","email":"ada@example.com","role":"admin","created_at":"2026-10-20T23:18:27.299365Z"}
$ curl -w ' %{http_code}\n' localhost:3000/users/99
{"error":"not found"} 404
$ curl -d '{"user_id":2,"title":"From curl"}' localhost:3000/posts
{"id":3,"user_id":2,"title":"From curl","body":null}
```

The bundle needs nothing beside it. Copy it anywhere and run it with the database's URL:

```sh
cp dist/server.mjs /tmp/app/ && cd /tmp/app
DATABASE_URL=postgres://localhost:5432/reylo_esbuild PORT=3000 node server.mjs
```

The `PORT` variable picks the port (3000 by default), and `DATABASE_URL` in the environment wins over `.env`. Running `nub run setup` starts the tables over.

Misspell a column in `queries/get_user.sql` and run `nub run typecheck` again: the error shows at that file's line and column, such as `queries/get_user.sql(1,12): error reylo6: column "nmae" does not exist (42703)`, and `nub run build` stops there.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency, because the bundle carries its run time (`defineQuery` and the driver adapters); esbuild, the Postgres engine `@reylosql/postgres` and `typescript` 7.1 are dev dependencies, used by the build and the type check only. The `postgres` package is the driver the server runs the queries with, and the type check reads the schema through it too.
- In `build.ts`, `plugins: [reylo()]` from `reylo/esbuild` turns each `.sql` import into a module that carries the statement, its parameters' order and types, and the row count Reylo proved, so the bundle reads no `.sql` file and asks the database nothing before it runs a query. The option `format: "esm"` is required for the top-level code of an ES module project; the build runs through `esbuild.build`, since esbuild runs plugins in `.build()` only and `.buildSync()` throws.
- In the `package.json` scripts, `typecheck` is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper. The `build` script runs that type check before `node build.ts`, because esbuild strips types without checking them; the plugin itself fails the build on a statement the database refuses, at its line and column. The `dev` script is `node --watch --env-file-if-exists=.env --import reylo/register src/server.ts`: the preload builds each `.sql` module as Node loads it, so development needs no build, and `--watch` restarts on a change to a `.sql` file too. The `start` script is `node --env-file-if-exists=.env dist/server.mjs`, with no preload.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for the project file, `.reylo.yaml` or `.reylo.yml`. The `include` option is `src`, `build.ts` and `setup.ts`, so `setup.sql`, a script rather than one query, stays out of the type check.
- The `.env` file sets `DATABASE_URL`, which Reylo reads for its analysis (the type check, the editor, the plugin and the preload) and the server reads through `--env-file-if-exists`.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported from `src/server.ts` with a relative path. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## Build without a database

A container or CI build that cannot reach the database builds from `.reylo/`, the analyses the type check, the plugin and `reylo generate` keep (see [In CI](https://reylo.dev/#in-ci)). Where the database can be reached, fill it:

```sh
DATABASE_URL=postgres://db.internal:5432/app npx reylo generate   # nubx reylo generate here
```

Carry `.reylo/` into the build with the sources (it ignores itself in git, so copy it, or run `reylo generate` in the same CI job), then build. The build needs no `DATABASE_URL`: one it cannot reach, or none, reads the analyses in `.reylo/`, whichever database they were read from.

```sh
npm run build
```

The type check and the plugin both read `.reylo/`, so both pass without the database, and the dev dependencies (esbuild, TypeScript and `@reylosql/postgres`) must be installed for the build. The image that runs the server needs only `node` and `dist/server.mjs`. A query that `.reylo/` lacks, such as one edited since `reylo generate` ran, fails the type check and the build at that file: with a `DATABASE_URL` it cannot reach, `.reylo has no analysis of this statement, so run reylo generate where the database can be reached`, and with none, `No database URL`.

## In your own project

- Install from npm instead of the `link:` dependencies: `npm install reylo postgres` and `npm install -D @reylosql/postgres esbuild typescript@beta @types/node`.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- Use `esbuild.build`: esbuild runs plugins in `build` only, and `buildSync` throws.
- esbuild's default format on `platform: "node"` is CommonJS, which fails here twice over: esbuild refuses a top-level `await` in CommonJS, and Node reads a `.js` file in a `"type": "module"` project as an ES module, where `require` is not defined. So the build sets `format: "esm"` and writes `.mjs`.
- postgres.js is an ES module and its bundle needs no `require`. A driver written in CommonJS, such as `pg`, does: bundled into ES module output it fails at run time with `Dynamic require of "events" is not supported` unless the build adds `banner: { js: 'import { createRequire } from "node:module"; const require = createRequire(import.meta.url);' }`.
- The bundle holds none of Reylo's analysis: no engine, no WebAssembly. That stays in the build.
