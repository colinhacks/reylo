# Reylo example: Node

A Node project that imports `.sql` files as typed queries and runs them on Postgres through postgres.js, with Reylo's preload loaded by `node --import reylo/register`. It is the setup of [Node](https://reylo.dev/#node) in the docs.

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/node
nub install                   # or pnpm install (npm does not take link: dependencies)
createdb reylo_example        # any Postgres database you can drop tables in
cp .env.example .env          # DATABASE_URL=postgres://localhost:5432/reylo_example
nub run setup                 # creates users and posts: "created users and posts"
nub run typecheck             # tsc --noEmit --runExternalCode: no output, exit 0
nub run start                 # runs src/main.ts
nub run test                  # node --test: 2 tests pass
```

Running `nub run start` prints ada, the post it creates, and the admin's posts:

```text
ada (admin) joined 2026-10-20T21:35:33.898Z
created #3 From main.ts
#1 Hello by ada
#2 Again by ada
#3 From main.ts by ada
```

Each run of `start` adds a post; `nub run setup` starts over. The tests roll their insert back. A `DATABASE_URL` in the environment wins over `.env`.

Misspell a column in `queries/get_user.sql` and run `nub run typecheck` again: the error shows at that file's line and column, such as `queries/get_user.sql(1,18): error reylo6: column "emial" does not exist (42703)`.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency, because the preload runs in the app; the Postgres engine `@reylosql/postgres` and `typescript` 7.1 are dev dependencies, used by the type check. The `postgres` package is the driver the app runs the queries with, and the type check reads the schema through it too.
- In the `package.json` scripts, `typecheck` is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper. The `start` script is `node --env-file-if-exists=.env --import reylo/register src/main.ts`: `--import reylo/register` turns each `.sql` import into a module as Node loads it, and `--env-file-if-exists` loads `.env` for the app. The `test` script is the same preload with `--test`, which Node passes to each test file's process, so `src/queries.test.ts` imports `.sql` files too.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for `.reylo.yaml`, the project file. The `include` option is `src` and `setup.ts`, so `setup.sql`, a script rather than one query, stays out of the type check.
- The `.env` file sets `DATABASE_URL`, which Reylo reads for its analysis (the type check, the editor and the preload) and the app reads through `--env-file-if-exists`.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported from `src/` with a relative path. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## In your own project

- Install from npm instead of the `link:` dependencies: `npm install reylo` and `npm install -D @reylosql/postgres typescript@beta`.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- Node finds `src/queries.test.ts` by its default pattern, `**/*.test.ts`, on Node 24 and 26; `--import` reaches each test file because Node starts the test processes with the flags of the run.
- Running `node --env-file=.env` fails when `.env` is missing, so the scripts use `--env-file-if-exists`, which lets a CI job set `DATABASE_URL` in the environment alone.
