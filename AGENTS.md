# Working on Reylo

This guide is for coding agents and contributors working in this repository: what is where, how to set up, how to make the usual changes, and what the checks expect. Read it whole before a change.

Reylo types SQL from the database itself. A project imports a `.sql` file, and TypeScript 7.1's content mapper gives back its parameters, columns, nullability, nested JSON shapes and cardinality, read from the server's own analysis; nothing is declared by hand. The analysis is a Rust engine, built to one WebAssembly component per database, which connects to the database itself and which the npm package `reylo` hosts. DuckDB runs inside the app, so its engine opens no database: the loader answers its needs through the project's own `@duckdb/node-api`.

A maintainer may keep local notes beside this file, which are never committed; when they are present, read them too, and where the two differ they win: @AGENTS.local.md

## What is where

| Path | What it is |
| --- | --- |
| `src/` | The npm package `reylo`, TypeScript: the content mapper, the run time (`.run()`, `.runSync()`, `.with()`), the bundler adapters, the preload `reylo/register` and the CLI `reylo generate` |
| `packages/<database>/` | The engine packages `@reylosql/postgres`, `@reylosql/mysql`, `@reylosql/sqlite` and `@reylosql/duckdb`: a `package.json`, a README, the committed engine component in `engine/` (jco's transpile of the component `engine/build.sh` builds), and `tooling.ts`, from which the build bundles the build-time half of `src/` into the package. Optional peers of `reylo` at its exact version |
| `engine/` | The analysis engine, Rust: `core/`, shared by every database, and one crate per database, none depending on another, each implementing core's `Database` trait ([engine/README.md](./engine/README.md)). Built for the loader, with the program that connects it to its database (`native/`), to one WebAssembly component per database (`wasip2/`), and linked natively by the oracle |
| `oracle/` | The soundness oracle (`reylo-oracle`) and `oracle/cases/`, the regression suite for everything the engine infers |
| `tests/` | The loader's node tests |
| `site/` | The docs site, reylo.dev: Vite, React Router and MDX; its design is docs-kit, copied into `site/docs-kit/` |
| `examples/` | One complete project per setup the docs describe, the models to copy when setting Reylo up elsewhere ([examples/README.md](./examples/README.md)); each is checked |
| `lab/` | End-to-end check projects: one per dialect, the bundlers, Jest, Next.js, the drivers and the driver matrix |
| `skills/using-reylo/` | The agent skill for projects that use Reylo; not instructions for work here, but kept current with what users see |
| `scripts/build.ts` | `nub run build`: writes every publishable package to `dist/` |
| `.github/workflows/` | `test.yml` (CI on every push and pull request), `release.yml` (the staged npm release, which a maintainer dispatches), `pullfrog.yml` |

### Where to change what

| To change | Edit | Prove it with |
| --- | --- | --- |
| A type, nullability, JSON shape or cardinality Reylo infers | The engine crate of that database (map below) | An oracle case, the engine's tests, the rebuilt components ([recipe](#an-inference-fix-in-the-engine)) |
| What the type check shows: the declaration, the hover, a diagnostic | `src/mapper-server.ts`, `src/module-types.ts`, `src/render.ts`, `src/doc-comment.ts` | `tests/compiled.test.ts`, `definition`, `doc-comment`, `render` |
| One analysis path and the `.reylo/` cache | `src/analyze.ts`; the engine's host is `src/engine.ts`, which runs each database's component (`src/component.ts`) and its connections in a worker (`src/engine-worker.ts`); the replay loop for a host that answers the engine itself, as the docs site's playground does, is `src/engine-core.ts` | `tests/analyze.test.ts`, `engines`, `fingerprint` |
| How the engine connects to a database: the URL's settings, TLS, a refused login, a lost server | `engine/native/src/` (`postgres.rs`, `mysql.rs`, `sqlite.rs`, `tls.rs`), over the clients of `engine/facade/` | The crate's tests, `tests/connect.test.ts`, `catalog-queries`, and `analyze`'s MySQL stand-in (`tests/mysql-stand-in.ts`) |
| How the type check reaches a DuckDB database: the file, read-only for each analysis, and the prepared statement's describe | `src/duckdb.ts`, through the project's `@duckdb/node-api` | `tests/duckdb.test.ts` |
| How `.run()` binds parameters and decodes values | `src/execute.ts`, `src/values.ts` (one representation per SQL type), `src/decode.ts`, `src/decode-mysql.ts`, `src/decode-sqlite.ts`, `src/decode-duckdb.ts`, `src/rows.ts` (structured parameters) | `tests/execute.test.ts`, `decode`, `values`, `representation`, `structured`, `rows`, `casts`, then `lab/driver-matrix` |
| Typed JSON parameters, and a query's input and output JSON Schema | `src/json-schema.ts` (the run time's validator), `src/schema.ts`; the types are `engine/core/src/schema_type.rs` and `schema.rs` | `tests/json-schema.test.ts`, then `lab/json-schema-suite` |
| The check of a SQLite statement on the app's own SQLite | `src/sqlite-drivers.ts` | `tests/sqlite-drivers.test.ts` |
| The run-time freshness check | `src/fresh.ts` | `tests/fresh.test.ts` |
| A bundler or runtime integration | `src/vite.ts`, `esbuild.ts`, `webpack.ts`, `turbopack.ts`, `next.ts`, `jest.ts`, `register.ts` (with `bridge.ts` and `register-worker.ts`), each of which loads the build-time half from the engine package (`src/engine-package.ts`, `src/tooling.ts`); what they emit is `src/loader.ts` | `tests/vite.test.ts`, `next`, `jest`, then the matching example and `lab/bundlers` |
| The project file `.reylo.yaml` | `engine/core/src/config.rs` and `src/project.ts`; a core change, so all four artifacts are rebuilt. A new key, or a new meaning for one, is a design decision: agree it with a maintainer before writing it | `tests/project.test.ts`, `project-file-module`, `strict-params`, `cli`; the docs' `04-configuration.mdx` |
| The placeholder lexer | `engine/core/src/lex.rs` and its port `src/lex.ts`, together | `tests/lex.test.ts` and the engine's tests |
| The CLI | `src/commands.ts`, which `src/cli.ts` starts | `tests/cli.test.ts` |
| The docs | `site/content/` | `nub run build` in `site/` ([recipe](#a-docs-change)) |

The live tests in these files skip without both test databases ([JavaScript](#javascript)). A change a user of Reylo would notice also updates the docs and the skill, `skills/using-reylo/SKILL.md`, in the same pull request.

The engine, crate by crate:

- **Postgres**, `engine/postgres/src/`: `postgres.rs` (the analysis, round by round), `infer.rs` (expression columns' types and nullability), `cardinality.rs` (how many rows a statement returns), `jsonshape.rs` (nested JSON shapes), `levels.rs` (which query an aggregate belongs to), `returning.rs`, `analyze.rs` (parse-tree facts: the statement's kind, locking, the tables it reads and writes, and which parameters may be null, as through `coalesce`), `pgtypes.rs` (the type registry), `stack.rs` (how deeply a statement may nest), `extension.rs` (the handshake with a server-side extension). The parser is `pg_query` (libpg_query, Postgres 18), patched from `engine/postgres/vendor/` into `engine/postgres/.patched/` by the prepare step.
- **MySQL**, `engine/mysql/src/`: `mysql.rs` (the analysis over the server's prepare metadata), `mysql_nulls.rs` (what a WHERE implies is not null), `mysql_derived.rs` (derived tables and WITH queries), `mysql_shape.rs` (JSON constructors), `mysql_params.rs` (which parameters may be null). It reads statements with its own copy of `sqlparser` 0.63, cut to the statements it needs (`engine/mysql/vendor/sqlparser`, whose README lists the changes).
- **SQLite**, `engine/sqlite/`: `src/sqlite.rs` (the analysis over SQLite's resolved tree, and its function tables), `src/sqlite_tree.rs` (the tree as `csrc/reylo_sqlite.c` serializes it), and `csrc/patch-sqlite.py` (the hooks the prepare step applies to the amalgamation).
- **DuckDB**, `engine/duckdb/src/`: `duckdb.rs` (the analysis over the prepared statement's types, which the host answers a describe with), `nulls.rs` (which columns are never NULL, and how many rows a statement returns), `catalog.rs` (the tables' NOT NULL columns and keys, from `duckdb_columns()` and `duckdb_constraints()`), `types.rs` (DuckDB's logical types as the engine's). It reads statements with `sqlparser` 0.63 from crates.io, whose DuckDB dialect reads DuckDB's own syntax (`FROM` first, `QUALIFY`, `EXCLUDE`); a statement it does not read is typed from the describe alone.
- **Core**, `engine/core/src/`: `database.rs` (the `Database` trait each database implements), `types.rs` (the type model), `shape.rs` (the nested-JSON vocabulary every database plans with), `lex.rs`, `config.rs`, `structured.rs` (`row(…)` and `rows(…)` parameters), `schema_type.rs` (a JSON Schema CHECK to a type), `schema.rs` (a type to JSON Schema), `api.rs` (the request protocol), `host.rs` (the answers a host gave and the needs the engine records), `fresh.rs`, `fingerprint.rs`.

The engine's core never opens a connection: it replays an analysis, names the statements to describe and the catalog queries to run, and its host answers them. The host the engine packages carry is Rust too: each component holds its database's program (`engine/native/`), which opens the connection itself, through tokio-postgres, mysql_async or rusqlite, and answers the core's needs, while a host that holds a connection of its own drives the core's replay itself (`src/engine-core.ts`). Postgres is the reference dialect; the docs' page for each dialect (`site/content/docs/<database>.mdx`) states what it can and cannot type.

## Setup

### JavaScript

Prefer nub over `node`, `bun`, npm, npx, pnpm and yarn: `nub install`, `nub add`, `nub run <script>`, `nub <file>` to run a file, `nubx <tool>` for a local CLI, and `nub --node <file>` for strict, unaugmented Node. It keeps the existing lockfiles. CI uses nub 0.9.6 on Node 24.

- `nub install` at the root, then `nub run typecheck` (`tsc --noEmit --runExternalCode`, about a second) and `nub run test` (`node --test 'tests/**/*.test.ts'`, about 15 seconds) are the loader's checks.
- CI fails on any skipped test: a run must end with `ℹ skipped 0`, and `nub run test 2>&1 | grep -E '^﹣|ℹ skipped'` lists each skip with its reason. `REYLO_TEST_PG` and `REYLO_TEST_MYSQL` name the test databases ([Test databases](#test-databases)); the Postgres tests of structured parameters, freshness and the schema fingerprint also need `lab/drivers` installed; the Jest, Vitest and SQLite-driver tests need `lab/drivers` and `lab/jest` installed; and the DuckDB tests need `lab/drivers` installed, whose `@duckdb/node-api` they link into each test project. The full setup, as CI makes it, from the repository's root:

  ```sh
  (cd lab/drivers && nub install) && (cd lab/jest && nub install)
  mkdir -p lab/jest/node_modules/@reylosql
  ln -sfn ../../.. lab/jest/node_modules/reylo && ln -sfn ../../../../packages/sqlite lab/jest/node_modules/@reylosql/sqlite
  ```

  Every lab reaches the package through a link, `node_modules/reylo`, to the repository root, and its engine through `node_modules/@reylosql/<database>` into `packages/`. `nub install` in a lab removes the links, so make them again after one.
- `nub run build` writes `dist/reylo` (the run time and the entry points, no engine) and `dist/postgres`, `dist/sqlite`, `dist/mysql` and `dist/duckdb` (the engine component, `tooling.js`, `engine-worker.js` and the I/O worker of jco's preview2-shim, which both bundle: nothing platform-specific, and no dependency), and refuses versions that disagree. The examples link `dist/` with `link:`, not the sources, so run it before `nub install` in an example and again after any change to `src/`. Never turn those links into `file:` dependencies: nub 0.9.6 never refreshes a `file:` directory it has installed once, so the example would run a stale build.
- `reylo` finds a project's engine from the project first, then from its own install (`src/engine.ts`, `src/engine-package.ts`), and refuses one of another version. The root's own tests reach the four engines through its `devDependencies` (`link:packages/<database>`), and DuckDB's tests reach `@duckdb/node-api` through `lab/drivers`, as an app's install would hold it.
- `site/` has its own install: `nub install` there before building or serving it.

### Rust

One Cargo workspace at the root covers `engine/core`, `engine/postgres`, `engine/sqlite`, `engine/mysql`, `engine/duckdb`, `engine/build-support`, `engine/facade` (`reylo-engine`, which a Rust host links: each database's engine behind a feature, and the answers to its needs over `tokio-postgres`, `rusqlite` or `mysql_async`), `engine/native` (`reylo-engine-native`, the program each component holds: the connection, TLS and the engine's requests), and `oracle/`: one `Cargo.lock` and one `target/`. The component crate, `engine/wasip2`, is a workspace of its own, so that the tokio-postgres patch it builds with reaches no native build. Building the components needs a rustup toolchain with the `wasm32-wasip2` target (`rustup target add wasm32-wasip2`; a distribution's `rustc` without it fails with "can't find crate for `std`"), wasi-sdk 34, named by `WASI_SDK_PATH` or unpacked under `engine/.wasi-sdk/`, and jco from the root's install.

```sh
engine/build.sh --prepare     # once per checkout, before any cargo command: cargo cannot resolve the workspace without it
cargo test -p reylo-engine-core -p reylo-engine-postgres -p reylo-engine-sqlite -p reylo-engine-mysql -p reylo-engine-duckdb -p reylo-engine -p reylo-engine-native -p reylo-oracle
engine/build.sh               # rebuilds all four packages/<database>/engine/
```

- The prepare step fetches `pg_query` and libpg_query at pinned commits, checks their SHA-256 and applies the patch in `engine/postgres/vendor/` into `engine/postgres/.patched/`, which the workspace's `[patch]` points at; it fetches the SQLite amalgamation into `engine/sqlite/.sqlite/` and applies the engine's hooks; and it fetches tokio-postgres from crates.io into `engine/wasip2/.patched/` and applies the patch beside it, which builds its runtime for wasip2.
- On macOS, bindgen needs the SDK: `export SDKROOT="$(xcrun --show-sdk-path)"`, then in a second `export`, `BINDGEN_EXTRA_CLANG_ARGS="-isysroot $SDKROOT"` (on one line the shell expands `$SDKROOT` before it is set, and bindgen fails with an opaque `libclang error`).
- **An engine change carries its rebuilt components.** `packages/<database>/engine/` is committed, so a checkout works without a Rust toolchain, and each component reports two hashes on ping: its engine's fingerprint, core's sources and its own database's (`engine/core/src/fingerprint.rs`), and `host`, the sources of the code that connects it (`engine/wasip2`, `engine/native`, `engine/facade`): the loader's tests fail when a committed component does not match its sources, and any edit under `engine/core/`, `engine/native/`, `engine/facade/` or `engine/wasip2/` changes all four.
- `engine/build.sh postgres` rebuilds one component. Each is built on its own, in a target directory of its own, so its bytes are those any build of it gives; the build writes no path of the machine that built it into the component.
- The components are built without LTO: the depth a statement may nest was measured on that code, and LTO's larger frames spend more of the JavaScript engine's stack at the bound.

### Test databases

The tests and the oracle create and drop databases, so point them only at throwaway servers, never at one whose data matters; the oracle refuses ports 5432 and 3306 outright. CI's servers, on ports clear of the defaults:

```sh
docker run -d --name reylo-pg -p 55432:5432 -e POSTGRES_HOST_AUTH_METHOD=trust postgres:18     # postgres://postgres@127.0.0.1:55432/postgres
docker run -d --name reylo-mysql -p 53306:3306 -e MYSQL_ALLOW_EMPTY_PASSWORD=yes mysql:8.4     # mysql://root@127.0.0.1:53306
```

- With a local Postgres install instead: `oracle/cluster.sh start <its bin directory> <port>` makes a throwaway cluster under `/tmp` and prints its URL, and `oracle/cluster.sh stop <port>` deletes it. On macOS a postmaster started with no locale in its environment stops at once ("postmaster became multithreaded during startup"), so set `LANG=en_US.UTF-8` for it.
- The oracle's `json_schema_*` cases call five JSON Schema validator extensions in Postgres 18. `.github/scripts/json-schema-validators.sh` builds them against a Postgres install's `pg_config`, as the oracle job in `test.yml` does.
- The oracle's `pgvector_types` and `postgis_types` cases need pgvector and PostGIS in that Postgres 18: on Debian and Ubuntu, PGDG's `postgresql-18-pgvector` and `postgresql-18-postgis-3`, as the oracle job installs them; with Homebrew's `postgresql@18`, `brew install pgvector postgis`.
- CI's Postgres cluster for the oracle is made with `initdb -E UTF8 --locale=C`, so a case that depends on collation can differ on a cluster with another locale.
- MySQL 9.7 and later start with GTIDs on, which refuses the oracle's MyISAM and MEMORY writes: start them with `--gtid-mode=OFF --enforce-gtid-consistency=OFF`. The `mysql_*` cases are blessed on MySQL 8.4.

## Recipes

### An inference fix in the engine

Most merged pull requests are this shape (colinhacks/reylo#10, colinhacks/reylo#29, colinhacks/reylo#33): the rule in `engine/<database>/src/`, a new oracle case, and the rebuilt artifact.

1. Prepare the workspace ([Rust](#rust)), and start a Postgres 18 server, plus MySQL 8.4 for a MySQL rule.
2. Write the failing case first: add statements to the closest existing case in `oracle/cases/`, or make a new directory when none covers the construct, with `setup.sql` (the tables), one statement per `<statement>.sql`, and a file named `dialect` holding `mysql`, `sqlite` or `duckdb` for those dialects (none for Postgres). Copy the layout of a neighbouring case.
3. Run it: `cargo run -q -p reylo-oracle -- --url <pg url> --mysql-url <mysql url> oracle/cases/<name>` (`ORACLE_URL` and `ORACLE_MYSQL_URL` can stand for the flags; a run needs the MySQL one only for a MySQL case). A DuckDB case needs no server, and an oracle built with DuckDB, which it compiles from DuckDB's sources in a few minutes: `cargo run -q -p reylo-oracle --features duckdb -- oracle/cases/<name>`. The same oracle replays DuckDB's own test suite, `test/sql` of DuckDB's repository at the version it builds, run from that repository's root: `--sqllogictest test/sql` holds what each of its queries returns against what the engine infers for it. Each line of `expected.txt` is a statement's analysis: `<file>: <cardinality> (<parameters>) -> { <columns> }`, a `?` marking a nullable type.
4. Fix the rule, then `--bless` the case to write its `expected.txt`, and read that file's diff: each changed line is a type change the pull request claims.
5. Run the whole oracle as CI does, `--strict-coverage oracle/cases`: a fix often changes other cases' `expected.txt`, and each of those changes must be right too. Then the engine's tests, `cargo build --workspace --all-targets` with no warning, and `engine/build.sh`.
6. Run the loader's checks (`nub run typecheck`, `nub run test` with both test databases), which compare the committed artifact with its sources.
7. Where the dialect's docs page states the rule (`site/content/docs/<database>.mdx`), correct it in the same pull request.

The oracle analyzes each statement through a live describe and catalog, then runs it in rolled-back transactions over seeded edge values, twice (with and without the values of extreme magnitude), under a 5000 ms timeout (`--timeout <ms>`, or a case's `timeout` file). A value that contradicts a type, or a row count beyond the cardinality, fails the case. A statement that failed or timed out on every run was checked for nothing: it is listed under `UNCHECKED`, a case's `unchecked.txt` holds those lines (empty where every statement must run; compared and blessed like `expected.txt`), and `--strict-coverage` fails a case that has such a statement and no `unchecked.txt`, which is how to run statements from elsewhere. A directory with no `expected.txt` is checked for soundness alone. A case may also carry a `min_version` file (skipped on an older server) and, for MySQL, a `sql_mode` file. The oracle creates and drops a database per case. MySQL statements run on sessions opened as mysql2 opens them, three ways: bound over the binary protocol as mysql2's `execute()` binds, through `SQL_BUFFER_RESULT`, and with their values written into the text as mariadb's `query()` and PlanetScale write them; a run that ends the transaction, writes a MyISAM or MEMORY table or moves an AUTO_INCREMENT counter far has the database built again (`oracle/src/mysql.rs`). `--dump <file>` writes every analysis and `--analyses <file>` checks another engine's.

### A loader change

The run time, the mapper, the adapters and the CLI live in `src/` (the table above names the file and its tests). Run `nub run typecheck` and `nub run test` with both test databases. A change to how values are bound or decoded is measured on every driver by `lab/driver-matrix` (its README has the commands; `compare.ts` checks a run row for row against `RESULTS.md`, which `./run.sh` rewrites). A change to an adapter or the preload is checked in its example: `nub run build`, then `nub install` in `examples/<name>` and its README's commands. The docs site's build imports `src/analyze.ts`, `src/render.ts` and `src/doc-comment.ts` and runs the mapper, so a change there also runs `nub run build` in `site/`. A change users see goes into the docs ([A docs change](#a-docs-change)) and the skill in the same pull request.

### A docs change

The docs are `site/content/docs/`: the main page `index.mdx`, which continues in one numbered file per h2 section in `site/content/sections/` (`01-setup.mdx` to `09-faq.mdx`), and one reference page per dialect (`postgres.mdx`, `mysql.mdx`, `sqlite.mdx`) with every rule and edge case of its database. reylo.dev serves the `release` branch, which each release moves to its tagged commit, so the docs describe what npm serves; a push to `main` builds a preview. A docs change that holds for the released version (a typo, a clearer sentence, a missing example) can go live before the next release: after it lands on `main`, cherry-pick its commit onto `release` and push, which only ever moves forward.

1. Every type and error shown on the page is captured, never typed by hand. After changing a `.sql` block or a page's schema, run `nub run capture` in `site/` with throwaway servers of both kinds: `REYLO_CAPTURE_POSTGRES=postgres://postgres@localhost:<pg port>/reylo_capture REYLO_CAPTURE_MYSQL=mysql://root@127.0.0.1:<mysql port>/reylo_capture` (the database is dropped and made again, so its name must start with `reylo_capture`). It rewrites `site/lib/sql-modules.json` and `site/lib/sql-errors.json`.
2. `nub run build` in `site/` type-checks and builds the site to `site/build/client`, and fails on a stale capture. `nub run dev --port <port> --strictPort` in `site/` serves it (3311 by default).
3. The voice is brief and instructive, as in Zod's docs: a sentence or two, then an example, and an example in every section; [Writing conventions](#writing-conventions) has the rest.

`site/docs-kit/` is a copy of a design shared with another site: change it at its source and sync it, never in place (`node docs-kit/src/sync.mjs . --check` in `site/` fails on a copy edited in place).

## Checks

`.github/workflows/test.yml` runs every check on each push to `main` and on each release, and on a pull request the checks its change reaches, its run for an older commit cancelled:

- **test**, on every pull request: on Node 24 with throwaway Postgres 18 and MySQL 8.4, `nub install` at the root, in `lab/drivers` and in `lab/jest`, the Jest lab's links, `nub run typecheck`, `nub run test` with none skipped, and `nub run build`.
- **engine**, on a pull request that changes `engine/`, `oracle/`, Cargo's files, `src/error-codes-*`, the JSON Schema validators' measurements the engine's tests read, or `.github/` (a renamed file at both its paths): `engine/build.sh --prepare`, `cargo build --workspace --all-targets` with no warning, and `cargo test` of the crates the change reaches (every one, `-p reylo-engine-core -p reylo-engine-postgres -p reylo-engine-sqlite -p reylo-engine-mysql -p reylo-engine-duckdb -p reylo-engine -p reylo-engine-native -p reylo-oracle`, where it changes core, `engine/facade/`, `engine/native/`, `engine/wasip2/`, the oracle's code, the build or CI; else the engine of each database it changes).
- **oracle**, on the same pull requests: `cargo run -q -p reylo-oracle -- --url <pg> --mysql-url <mysql> --strict-coverage <cases>`, in shards side by side (Postgres's cases, SQLite's, DuckDB's (with `--features duckdb`), MySQL's in two, and `mysql_types`, the slowest, alone), each database's only where the change reaches it (`engine/<database>/` or one of its cases; everything for the files that run every engine check). Postgres 18 has pgvector, PostGIS and the five JSON Schema validators, which `.github/scripts/json-schema-validators.sh` builds and a cache keeps, with the packages of Postgres itself; MySQL is 8.4. A failure in the step that builds the validators is not the engine's.

Run what CI runs before you push. A failing check's log: `gh pr checks <number> -R colinhacks/reylo` names the run, and `gh run view <run id> -R colinhacks/reylo --log-failed` prints what failed. Pullfrog and Copilot review every pull request, and Vercel deploys a preview of the docs site.

## Pull requests and commits

- **One change per pull request**, never several unrelated ones batched together. A pull request is squash-merged, so it lands as one commit whose subject is its title.
- **A pull request that changes the engine carries its rebuilt components** (`packages/<database>/engine/`). When another such pull request lands first, rebase on `origin/main`, rebuild them and force-push the branch.
- **Commit messages and pull request titles are Conventional Commits**: `type(scope): subject`, at most 72 characters, the subject imperative and lowercase with no trailing period, as `fix(mysql): type left-joined columns as nullable`. The type is `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci` or `chore` (`chore(release): 0.2.0` for a release); the optional scope is `engine`, `postgres`, `mysql`, `sqlite`, `duckdb`, `mapper`, `cli`, `site`, `docs-kit`, `examples`, `oracle`, `readme` or `release`. A breaking change takes `!` after the scope. Only when the subject alone hides something, a blank line and a few short lines say why.
- **Committed text states rules, not people.** A code comment, a test's name, an error message, a README, a docs page, a commit message or a pull request never names or quotes a person, or gives a reason as someone's word: it states the rule and its reason itself.
- **Commit each finished, verified step** on the branch, rather than one commit for a session's work.

## Writing conventions

- Never hard-wrap Markdown or MDX: every paragraph, list item and table row is one line.
- A frontmatter `description:` carries no inline code.
- Prose in the docs never opens a sentence with inline code, writes a method as `.run()` (never a bare `run`), says Reylo infers a type (never that it proves one), and says "cardinality" for how many rows a query returns.
