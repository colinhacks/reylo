# Reylo example: Rspack

A small Node HTTP server that imports `.sql` files as typed queries and runs them on Postgres through pg, bundled by Rspack 2 into one ES module, `dist/server.mjs`, that runs with `node` and no preload. It is the setup of [Rspack](https://reylo.dev/#setup-rspack) in the docs.

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/rspack
nub install                   # or pnpm install (npm does not take link: dependencies)
createdb reylo_example        # any Postgres database you can drop tables in
cp .env.example .env          # DATABASE_URL=postgres://localhost:5432/reylo_example
nub run setup                 # creates users and posts: "created users and posts"
nub run typecheck             # tsc --noEmit --runExternalCode: no output, exit 0
nub run dev                   # the sources under the preload, restarted on every change, .sql files included
nub run build                 # rspack build: "Rspack compiled successfully", writes dist/server.mjs
nub run start                 # node dist/server.mjs, on http://localhost:3000 (PORT changes it)
```

The server answers three routes, the same in `dev` and from the bundle:

```text
$ curl localhost:3000/posts
[{"id":1,"title":"Hello","body":"first post","author":"ada"},{"id":2,"title":"Again","body":null,"author":"ada"}]
$ curl localhost:3000/users/1
{"id":1,"name":"ada","email":"ada@example.com","role":"admin","created_at":"2026-10-20T23:18:31.828199Z"}
$ curl localhost:3000/users/99
{"error":"no such user"}
$ curl -X POST -d '{"user_id":2,"title":"From bob"}' localhost:3000/posts
{"id":3,"user_id":2,"title":"From bob","body":null}
```

The route `/users/99` answers 404, and the `POST` 201 with the row the insert returned. Each `POST` adds a post; `nub run setup` starts over. A `DATABASE_URL` in the environment wins over `.env`. The bundle `dist/server.mjs` carries pg and Reylo's run time, so it runs from any directory, without `node_modules`.

Misspell a column in `queries/get_user.sql` and run `nub run typecheck` again: the error shows at that file's line and column, such as `queries/get_user.sql(1,12): error reylo6: column "nmae" does not exist (42703)`. The build, `nub run build`, fails on it too, with `queries/get_user.sql:1:12: column "nmae" does not exist (42703)`.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency, because each `.sql` module imports its run time, which Rspack bundles into `dist/server.mjs`. The Postgres engine `@reylosql/postgres` and `typescript` 7.1 are dev dependencies, used by the type check and by the build. The `pg` package is the driver the app runs the queries with; the type check and the build read the schema through the postgres.js inside the engine.
- In `rspack.config.mjs`, the rule `{ test: /\.sql$/, use: "reylo/webpack" }` turns each `.sql` import into a module that runs the statement. The loader analyzes the statement at build time, against the database `DATABASE_URL` names, so the bundle needs neither the database's schema nor a preload when it runs. The rest of the file is Rspack's own setup for a Node server: `target: "node24"`, `output.module` for an ES module, `builtin:swc-loader` for TypeScript, and pg's optional `pg-native` left out.
- In the `package.json` scripts, `typecheck` is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper. The `build` script is `rspack build`, which needs nothing from Reylo but the rule. The `start` script is `node --env-file-if-exists=.env dist/server.mjs`, with no preload. The `dev` script runs the sources with `node --watch --env-file-if-exists=.env --import reylo/register src/server.ts`: Node strips the types, `--import reylo/register` turns each `.sql` import into a module as Node loads it, and `--watch` restarts on a change to a `.sql` file too.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for the project file, `.reylo.yaml` or `.reylo.yml`. The `include` option is `src`, `queries` and `setup.ts`, so every query is checked, imported or not, and `setup.sql`, a script rather than one query, stays out. The options `verbatimModuleSyntax` and `erasableSyntaxOnly` keep the sources to what Node's type stripping runs in `dev`.
- The `.env` file sets `DATABASE_URL`, which Reylo reads for its analysis (the type check, the editor, the build and the preload) and the app reads through `--env-file-if-exists`.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported from `src/server.ts` with a relative path. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## In your own project

- Install from npm instead of the `link:` dependencies: `npm install reylo pg` and `npm install -D @reylosql/postgres @types/pg typescript@beta`.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- pg requires `pg-native` inside a `try`, for `pg.native` only, and Rspack warns that it cannot resolve it. Rspack's `IgnorePlugin` leaves it out; an app that uses `pg.native` installs `pg-native` and lists it in `externals` instead, since a native addon cannot be bundled.
- The loader `builtin:swc-loader` strips the types and takes its syntax target from `target`, so the bundle keeps Node 24's syntax; `tsc` checks the types.
- The `typescript` package is pinned to `7.1.0-beta`: a range such as `^7.1.0-beta` also takes the nightlies, `7.1.0-dev.*`, which semver orders above the beta.
