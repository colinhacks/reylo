# @reylosql/mysql

Reylo's analysis engine for MySQL, which [reylo](https://www.npmjs.com/package/reylo) loads to type the `.sql` files of a project whose database is MySQL. It holds the engine, built to WebAssembly, and the build steps that run it, which connect to MySQL through mysql2, carried inside the package. The run time and the entry points a project names are reylo's, which runs the type check, the bundler plugins and the CLI from this package.

```sh
npm install reylo
npm install -D @reylosql/mysql
```

Install it at the version of reylo: reylo refuses an engine of another version. A project with more than one database installs the engine of each.
