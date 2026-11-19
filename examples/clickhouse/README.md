# Reylo example: ClickHouse

A Node project that imports `.sql` files as typed queries and runs them on ClickHouse through `@clickhouse/client`, with Reylo's preload loaded by `node --import reylo/register`. It is the setup of [Node](https://reylo.dev/#setup-node) in the docs, on the database of [ClickHouse](https://reylo.dev/clickhouse).

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/clickhouse
nub install                   # or pnpm install (npm does not take link: dependencies)
docker run -d --name reylo-example-clickhouse -p 8123:8123 -e CLICKHOUSE_PASSWORD=reylo clickhouse/clickhouse-server:26.8   # or any ClickHouse 25.11 or later with a database you can drop tables in
cp .env.example .env          # DATABASE_URL=clickhouse+http://default:reylo@localhost:8123/default
nub run setup                 # creates users and posts: "created users and posts"
nub run typecheck             # tsc --noEmit --runExternalCode: no output, exit 0
nub run start                 # runs src/main.ts
nub run test                  # node --test: 2 tests pass
```

Running `nub run start` prints ada, the post it creates, and the admin's posts:

```text
ada (admin) joined 2026-11-19T18:08:27.032Z
created c69b7133-0e7e-4f0a-8af4-2969cc7072c1 From main.ts
Hello by ada
Again by ada
From main.ts by ada
```

Each run of `start` adds a post; `nub run setup` starts over. The tests delete the post they insert, since ClickHouse has no transaction to roll back. A `DATABASE_URL` in the environment wins over `.env`.

Misspell a column in `queries/get_user.sql` and run `nub run typecheck` again: the error shows at that file with ClickHouse's own message, such as ``queries/get_user.sql(1,1): error reylo6: Unknown expression identifier `emial`. Maybe you meant: ['email'].``, then the statement and the error's name, `UNKNOWN_IDENTIFIER`. ClickHouse names no position for this error, so it marks the statement from its start.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency, because the preload runs in the app; `typescript` 7.1 is a dev dependency, used by the type check, and `reylo` brings the engine that analyzes each statement for the machine. The `@clickhouse/client` package is the client the app runs the queries with; the type check asks ClickHouse over the engine's own HTTP connection.
- In the `package.json` scripts, `typecheck` is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper. The `start` script is `node --env-file-if-exists=.env --import reylo/register src/main.ts`: `--import reylo/register` turns each `.sql` import into a module as Node loads it, and `--env-file-if-exists` loads `.env` for the app. The `test` script is the same preload with `--test`, which Node passes to each test file's process, so `src/queries.test.ts` imports `.sql` files too.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for the project file, `.reylo.yaml` or `.reylo.yml`. The `include` option is `src` and `setup.ts`, so `setup.sql`, a script rather than one query, stays out of the type check.
- The `.env` file sets `DATABASE_URL`, which Reylo reads for its analysis (the type check, the editor and the preload): the URL of ClickHouse's HTTP interface, after `clickhouse+`. The app reads it through `--env-file-if-exists` and gives its client the URL without that prefix, as `src/main.ts` does.
- The `queries/*.sql` files hold one statement per file, imported from `src/` with a relative path. Each parameter declares its type as ClickHouse writes one, `{id:UInt32}`, and one declared an `Enum8` takes the enum's names alone. The query `get_user.sql` ends in `LIMIT 1`, since ClickHouse enforces no unique key, and resolves to a row or `null`; `list_posts.sql` resolves to an array; `create_post.sql` and `delete_post.sql` are writes, which resolve to nothing, since ClickHouse has no `RETURNING`.
- A post's `id` is a `UUID` the app makes before the insert, since ClickHouse has no serial column to make one.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## In your own project

- Install from npm instead of the `link:` dependencies: `npm install reylo @clickhouse/client` and `npm install -D typescript@beta`.
- An app whose queries run on another server than the one its type check reads keeps a variable of its own for the client, beside `DATABASE_URL`.

## Notes

- Node finds `src/queries.test.ts` by its default pattern, `**/*.test.ts`, on Node 24 and 26; `--import` reaches each test file because Node starts the test processes with the flags of the run.
- Running `node --env-file=.env` fails when `.env` is missing, so the scripts use `--env-file-if-exists`, which lets a CI job set `DATABASE_URL` in the environment alone.
- ClickHouse's HTTP interface runs one statement a request, so `setup.ts` sends `setup.sql` a statement at a time.
