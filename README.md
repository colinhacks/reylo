<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="site/public/readme-logo-dark.svg" />
    <img src="site/public/readme-logo-light.svg" width="160" alt="Reylo" />
  </picture>
</h1>

<p align="center">Import <code>.sql</code> files into your TypeScript code with full type inference</p>

<p align="center">
  <a href="https://github.com/colinhacks/reylo/actions/workflows/test.yml"><img src="https://github.com/colinhacks/reylo/actions/workflows/test.yml/badge.svg?branch=main" alt="test status" /></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/github/license/colinhacks/reylo?style=flat" alt="License: MIT" /></a>
  <a href="https://www.npmjs.com/package/reylo"><img src="https://img.shields.io/github/v/release/colinhacks/reylo?label=npm" alt="npm version" /></a>
  <a href="https://github.com/colinhacks/reylo"><img src="https://img.shields.io/github/stars/colinhacks/reylo" alt="GitHub stars" /></a>
</p>

<div align="center">
  <a href="https://reylo.dev">Read the docs →</a>
</div>

<br/>

If you have a file like this:

```sql
-- get_author.sql
select u.id, u.name, count(p.id)::int as post_count
from users u
left join posts p on p.user_id = u.id
where u.id = :id::int
group by u.id
```

You can import it directly from a TypeScript file like this:

```ts
// main.ts
import postgres from "postgres";
import getAuthor from "./get_author.sql";

const db = postgres(process.env.DATABASE_URL);

const author = await getAuthor.run(db, { id: 1 });
// { id: number; name: string; post_count: number } | null
```

Reylo is powered by TypeScript [content mappers](https://github.com/microsoft/typescript-go/pull/4712), introduced in TypeScript 7.1 (currently [in beta](https://devblogs.microsoft.com/typescript/announcing-typescript-7-1-beta/)), which let you dynamically define static types for external file types and integrate them directly into TypeScript compilation.

Reylo is the perfect option for the agent era:

- 💪 **The full power of SQL** — any statement, function, operator or syntax your database supports, in its own dialect.
- 🌵 **DRY** — the types come from the SQL itself: no hand-written row types, no schema file to keep in sync.
- 🎯 **Checked against your database** — a misspelled column, a wrong parameter or a dropped table is a type error, in `tsc` and in your editor.
- 🤖 **Agent-friendly** — each fact lives in one place, so an agent cannot leave a stale type behind, and the type check finds every caller it missed.
- 🧩 **Postgres, MySQL and SQLite** — bring your own driver: `pg`, postgres.js, `mysql2`, `better-sqlite3`, Cloudflare D1 and [more](#supported-drivers).
- ⚡ **No codegen** — the type checker asks Reylo directly: no generated files, no watcher.

## Setup

To install Reylo, you'll need:

- **TypeScript 7.1 (currently in beta).** The first release with content mappers.
- **Node 24 or later.**
- **A running dev database.** Postgres, MySQL 8.0.22 or later, or a SQLite file: Reylo reads your current schema from it.

Install Reylo, the engine for your database and TypeScript 7.1:

```sh
npm install reylo
npm install -D @reylosql/postgres  # or mysql, sqlite
npm install -D typescript@beta
```

Add Reylo to `tsconfig.json`, for `.sql` files and for its project file, `.reylo.yaml` or `.reylo.yml`:

```jsonc
// tsconfig.json
{
  "compilerOptions": { /* ... */ },
  "contentMappers": [
    { "package": "reylo", "extensions": [".sql", ".reylo.yaml", ".reylo.yml"] }
  ]
}
```

Then type-check with `--runExternalCode`, which lets `tsc` run content mappers:

```jsonc
// package.json
{
  "scripts": {
    "typecheck": "tsc --noEmit --runExternalCode"
  }
}
```

Point Reylo at your dev database with `DATABASE_URL`, in your environment or in `.env`. Its scheme tells Reylo which database you use:

```sh
# .env
DATABASE_URL=postgres://localhost:5432/app  # or mysql://…, or file:./app.db for SQLite
```

To make `.sql` imports work at run time too, load Reylo where your code runs. On Node:

```sh
node --env-file=.env --import reylo/register main.ts
```

For Bun, Nub, Vite, Next.js, esbuild, webpack, Rspack, Cloudflare Workers and Jest, add [the plugin for your setup](https://reylo.dev/#plugins), or copy its complete [example project](https://github.com/colinhacks/reylo/tree/main/examples). To see the types as you type, set up [your editor](https://reylo.dev/#editors).

## Usage

Write a query in a `.sql` file, import it, and run it with your database driver:

```sql
-- get_user.sql
select id, name, email from users where id = :id::int
```

```ts
import getUser from "./get_user.sql";

const user = await getUser.run(db, { id: 1 });
// { id: number; name: string; email: string | null } | null
```

Name each parameter with a colon, and give it a type with a cast where the query doesn't pin one down. Pass `.run()` your driver's client: a pool, a client, or a connection or transaction taken from one.

It resolves to the rows, shaped by how many the query can return:

| The query | Resolves to |
| --- | --- |
| A match on a primary or unique key, or `limit 1` | `Row \| null` |
| An aggregate without `group by`, a one-row `insert … returning` | `Row` |
| An `insert`, `update` or `delete` without `returning` | `void` |
| Anything else | `Row[]` |

An imported query also has `.runSync()`, `.with()`, `.sql`, `.raw`, `.inputSchema` and `.outputSchema` ([Usage](https://reylo.dev/#usage)).

## What Reylo infers

To type a query, Reylo has your dev database prepare it, without running it, and reads back the type of every column and parameter. On top of that, it infers:

- **Nulls** — a column is nullable unless its schema, a join or a `where` test rules null out.
- **Cardinality** — a match on a unique key is one row or `null`, and an aggregate is exactly one row.
- **Unions** — an enum, or a column whose check constraint lists its values, is a union of them.
- **JSON** — JSON the query builds, and a JSON column with a JSON Schema check, keep their shape.

Each has examples in [Inference](https://reylo.dev/#inference).

## Supported drivers

Reylo is bring-your-own-driver. Pass `.run()` a client from any of these:

- **Postgres** — `pg`, postgres.js, PGlite, Neon, `pg-promise`, `@vercel/postgres`
- **MySQL** — `mysql2`, PlanetScale
- **SQLite** — `better-sqlite3`, `node:sqlite`, libSQL, Turso, Cloudflare D1, `sqlite3`, `sqlite`

## Documentation

[reylo.dev](https://reylo.dev) covers the rest: every plugin and editor, configuration, errors, each database's data types and edge cases, and advanced examples. Agents can read it all as Markdown at [reylo.dev/llms.txt](https://reylo.dev/llms.txt).

## License

[MIT](LICENSE)
