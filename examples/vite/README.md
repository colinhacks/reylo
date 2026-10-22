# Reylo example: Vite (React Router)

A React Router app on Vite whose loaders and action run `.sql` files as typed queries against Postgres through `pg` (node-postgres). Vite runs server code only through a framework, so React Router (framework mode) is the server here; Reylo's part is the Vite plugin from [Setup: Vite](https://reylo.dev/#setup-vite).

## Run it

```sh
(cd ../.. && nub run build)  # once, at the repository root: the built packages in dist/ this example links
nub install                  # or pnpm install (npm refuses link: dependencies)
createdb reylo_vite          # any Postgres; then point DATABASE_URL at it
cp .env.example .env
nub run setup                # node setup.ts: prints "created users and posts"
nub run typecheck            # react-router typegen && tsc --noEmit --runExternalCode; prints nothing
nub run test                 # vitest run: Tests 3 passed (3)
nub run dev                  # http://localhost:5173: the posts, /users/1 for ada, /users/999 a 404
nub run build                # react-router build: build/client and build/server
nub run start                # react-router-serve ./build/server/index.js on http://localhost:3000
```

The form on `/` posts to the home route's action, which inserts a post and shows the list again (`curl -d user_id=1 -d title=Hi -d body= 'http://localhost:3000/?index'`). While `nub run dev` runs, an edit to a `.sql` file is picked up on the next request: Vite reloads the server modules and the page, with no restart.

Misspell a column in `app/.server/queries/get_user.sql` and `nub run typecheck` reports it at the file's line and column:

```text
app/.server/queries/get_user.sql(1,12): error reylo6: column "nmae" does not exist (42703)
```

## How Reylo is wired in

- In `package.json`, `reylo` is a dependency (the plugin, and the runtime each query module imports); `@reylosql/postgres` (the engine that analyzes each statement), `typescript` 7.1 beta and `pg` (the driver, which the analysis also connects through) are installed beside it. The `typecheck` script runs `react-router typegen` first, for the routes' `./+types/*` modules, then `tsc --noEmit --runExternalCode`, which runs the content mapper. The build, `react-router build`, runs no type check of its own.
- In `tsconfig.json`, `contentMappers` names `reylo` for `.sql` and for `.reylo.yaml`, the project file. React Router's template includes `**/*` and `**/.server/**/*`, which takes in the queries under `app/.server/` and also `setup.sql`, so `exclude` drops `setup.sql`, a schema the type check would report as not one query.
- In `vite.config.ts`, `reylo()` sits beside `reactRouter()`. It turns each `.sql` import into a module carrying the analyzed statement, in the dev server and in both builds. Either order works: `reylo()` sets `enforce: "pre"`, which Vite runs before its own plugins whatever the list says, and no React Router plugin loads a `.sql` file. It comes first here because it reads as the step that runs first.
- Vitest reads `vitest.config.ts` instead of `vite.config.ts`, and it has only `reylo()`, since the tests import queries and no routes. Its `test.env` loads `.env` into `process.env` for the tests' pool, as React Router's plugin does for the dev server and the build.
- The module `app/.server/db.ts` holds the `pg.Pool` every query runs on.
- The queries sit in `app/.server/queries/*.sql`. React Router's build fails when anything under a `.server/` directory is reached from browser code, `.sql` files included, so a statement cannot end up in the browser bundle.
- The routes `app/routes/home.tsx` and `app/routes/user.tsx` each import their queries and run them only in `loader` and `action`, which React Router removes from the browser bundle together with the imports only they use.
- The test file `tests/queries.test.ts` runs the three queries against the database; the insert runs in a transaction that is rolled back.
- The `.vscode/` and `.zed/` directories hold the editor settings from [Editors](https://reylo.dev/#editors).

## In your own project

- Run `npm install reylo pg` and `npm install -D @reylosql/postgres typescript@beta` instead of the `link:` dependencies on `../../dist/`.
- Nothing else here exists only because Reylo is linked from this repository.

## Notes

- **Keep queries under `.server/`.** Outside it, a `.sql` file imported by browser code builds without a word: the client chunk then carries the statement's text, its analysis and Reylo's runtime, which cannot run there. Under `app/.server/`, `react-router build` stops with "Server-only module referenced by client", and the dev server answers the route's browser module with the same error. React Router's other form, a `.server` file name, does not cover `.sql` files: it matches `.server`, `.server.ts` and the like only.
- **The `.env` file**: React Router's plugin loads `.env` into `process.env` for `react-router dev` and `react-router build`, with the environment winning, and `vitest.config.ts` does the same for the tests; `setup` loads it with `node --env-file-if-exists=.env`. The server `react-router-serve` loads none, so `start` needs `DATABASE_URL` in the environment, as a host gives it (nub happens to hand `.env` to the Node process it starts, so `nub run start` works here with `.env` alone; `pnpm start` does not). The module `app/.server/db.ts` throws when it is unset, where `pg` would connect to localhost:5432 instead.
