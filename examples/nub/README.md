# Reylo example: nub

A [nub](https://github.com/nubjs/nub) project that imports `.sql` files as typed queries and runs them on SQLite through `node:sqlite`, with Reylo's preload loaded from `nub.jsonc`. It is the setup of [nub](https://reylo.dev/#setup-nub) in the docs.

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/nub
nub install                   # or pnpm install (npm does not take link: dependencies)
cp .env.example .env          # DATABASE_URL=file:./app.db
nub run setup                 # creates app.db from setup.sql: "created app.db"
nub run typecheck             # tsc --noEmit --runExternalCode: no output, exit 0
nub run start                 # nub src/main.ts
nub run test                  # nub --test: 2 tests pass
```

Running `nub run start` prints ada, the post it creates, and the admin's posts:

```text
ada (admin) joined 2026-10-20 21:37:58
created #3 From main.ts
#1 Hello by ada
#2 Again by ada
#3 From main.ts by ada
```

Each run of `start` adds a post; `nub run setup` starts over. The tests roll their insert back.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency, because the preload runs in the app; the SQLite engine `@reylosql/sqlite` and `typescript` 7.1 are dev dependencies, used by the type check. SQLite needs no driver package: `node:sqlite` is built into Node.
- The `nub.jsonc` file sets `"preload": ["reylo/register"]`, which nub loads before the app, so `nub src/main.ts` and `nub --test` import `.sql` files with no flag.
- In the `package.json` scripts, `typecheck` is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper. The `start` script is `nub src/main.ts` and `test` is `nub --test`, Node's test runner under nub, which finds `src/queries.test.ts` and runs it with the preload.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for the project file, `.reylo.yaml` or `.reylo.yml`. The `include` option is `src` and `setup.ts`, so `setup.sql`, a schema rather than one query, stays out of the type check.
- The `.env` file sets `DATABASE_URL=file:./app.db`, the SQLite file Reylo analyzes each statement against, its path starting at `package.json`. Reylo reads it for the type check, the editor and the preload, and nub loads `.env` on its own. The script `src/main.ts` opens the same file, `app.db`.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported from `src/` with a relative path. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote; `src/main.ts` runs it with `.runSync()`, which `node:sqlite` allows.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## In your own project

- Install from npm instead of the `link:` dependencies: `nub add reylo` and `nub add -D @reylosql/sqlite typescript@beta`.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- Setting `NODE_COMPAT=1` in the environment makes nub run as plain Node, which skips `nub.jsonc`: a `.sql` import then fails with `ERR_UNKNOWN_FILE_EXTENSION`. A process started under nub's `node` shim passes it on, so unset it if a `.sql` import stops loading.
- The preload analyzes each `.sql` file as it loads, against `DATABASE_URL` in `.env` or the environment, as the type check does.
- SQLite infers no union from a `check` constraint, so `role` is a `string` here, where Postgres's enum gives `"admin" | "member"`.
