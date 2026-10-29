# @reylosql/duckdb

Reylo's analysis engine for DuckDB, which [reylo](https://www.npmjs.com/package/reylo) loads to type the `.sql` files of a project whose database is DuckDB. It holds the engine, built to one WebAssembly component, and the build steps that run it, with the WebAssembly host they run it on: nothing in it is native to a platform, and it installs no dependency. DuckDB runs inside the app, so the type check prepares each statement on the project's own [`@duckdb/node-api`](https://www.npmjs.com/package/@duckdb/node-api), which the project installs. The run time and the entry points a project names are reylo's, which runs the type check, the bundler plugins and the CLI from this package.

```sh
npm install reylo @duckdb/node-api
npm install -D @reylosql/duckdb
```

Install it at the version of reylo: reylo refuses an engine of another version. A project with more than one database installs the engine of each.
