# Reylo example: Bun

A Bun project that imports `.sql` files as typed queries and runs them on SQLite through `bun:sqlite`, with Reylo's preload loaded from `bunfig.toml` and a bundle built by `Bun.build` with Reylo's esbuild plugin. It is the setup of [Bun](https://reylo.dev/#setup-bun) in the docs.

## Run it

```sh
nub run build                 # at the repository root, once: writes dist/, which this example links
cd examples/bun
nub install                   # or pnpm install (npm does not take link: dependencies)
cp .env.example .env          # DATABASE_URL=file:./app.db
nub run setup                 # bun setup.ts: creates app.db from setup.sql, "created app.db"
nub run typecheck             # tsc --noEmit --runExternalCode: no output, exit 0
nub run start                 # bun src/main.ts
nub run test                  # bun test: 2 pass
nub run build                 # bun build.ts: "built …/dist/main.js"
nub run start:dist            # bun dist/main.js: the bundle, the same output as start
```

Running `nub run start` prints ada, the post it creates, and the admin's posts:

```text
ada (admin) joined 2026-10-20 21:39:41
created #3 From main.ts
#1 Hello by ada
#2 Again by ada
#3 From main.ts by ada
```

Each run of `start` adds a post; `nub run setup` starts over. The tests roll their insert back. The same scripts run with `bun run <script>`.

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency, because the preload runs in the app; the SQLite engine `@reylosql/sqlite`, `typescript` 7.1 and Bun's types `@types/bun` are dev dependencies, used by the type check. SQLite needs no driver package: `bun:sqlite` is built into Bun.
- The `bunfig.toml` file sets `preload = ["reylo/register"]`, which Bun loads before the app, so `bun src/main.ts` imports `.sql` files with no flag; and the same line under `[test]`, because `bun test` reads its own preload and not the top-level one.
- The `build.ts` file runs `Bun.build` with `plugins: [reylo()]` from `reylo/esbuild`, which builds each `.sql` import into the bundle at build time, analyzed against the database. The bundle `dist/main.js` then carries the statements, Reylo's runtime and nothing else it needs: it runs without the preload, `node_modules` or `.env`.
- In the `package.json` scripts, `typecheck` is `tsc --noEmit --runExternalCode`, the flag that lets `tsc` run the content mapper. The `start` script is `bun src/main.ts`, `test` is `bun test`, `build` is `bun build.ts` and `start:dist` is `bun dist/main.js`.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql`, which types each import from the database, and for `.reylo.yaml`, the project file; `types: ["bun"]` loads Bun's types, which TypeScript 7 loads only when named. The `include` option is `src`, `setup.ts` and `build.ts`, so `setup.sql`, a schema rather than one query, stays out of the type check.
- The `.env` file sets `DATABASE_URL=file:./app.db`, the SQLite file Reylo analyzes each statement against, its path starting at `package.json`. Reylo reads it for the type check, the editor, the preload and the build, and Bun loads `.env` on its own. The script `src/main.ts` opens the same file, `app.db`.
- The `queries/*.sql` files hold one statement per file, with `:name` parameters, imported from `src/` with a relative path. The query `get_user.sql` resolves to a row or `null`, `list_posts.sql` to an array, and `create_post.sql`, an insert with `returning`, to the row it wrote; `src/main.ts` runs it with `.runSync()`, which `bun:sqlite` allows.
- In `.vscode/settings.json` and `.vscode/extensions.json`, the TypeScript 7 extension is pointed at this project's TypeScript, so the editor shows the queries' types and errors ([Editors](https://reylo.dev/#editors)). In Zed, `.zed/settings.json` does the same.

## In your own project

- Install from npm instead of the `link:` dependencies: `bun add reylo` and `bun add -d @reylosql/sqlite typescript@beta @types/bun`.
- The `.npmrc` file (`minimumReleaseAge=0`) exists only because this machine holds fresh releases back; drop it.

## Notes

- Without the `[test]` preload, `bun test` imports each `.sql` file as its path, a string, and the tests fail with a `TypeError` such as `createPost.runSync is not a function`.
- Running `bun dist/main.js` still loads the preload of `bunfig.toml` in this directory, though the bundle never uses it: copied with `app.db` into an empty directory, `bun main.js` prints the same rows.
- SQLite infers no union from a `check` constraint, so `role` is a `string` here, where Postgres's enum gives `"admin" | "member"`.
