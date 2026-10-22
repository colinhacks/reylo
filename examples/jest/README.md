# Reylo example: Jest

A small project whose Jest tests, written in TypeScript, run its `.sql` queries against a SQLite file through better-sqlite3, with Reylo's transformer `reylo/jest` turning each `.sql` import into a typed query. It is the setup of [reylo.dev/#setup-jest](https://reylo.dev/#setup-jest), in both of Jest's module modes.

## Run it

```sh
nub run build              # at the repository root, once: the example links the built packages in dist/

cd examples/jest
nub install                # or pnpm install (npm does not take link: dependencies)
cp .env.example .env       # DATABASE_URL=file:./app.db
nub run setup              # creates app.db from setup.sql: ada and bob, two posts by ada
nub run typecheck          # tsc --noEmit --runExternalCode, over the app, the tests and setup.ts
nub run test               # Jest with ES modules: "Tests: 3 passed, 3 total"
nub run test:cjs           # Jest with CommonJS: "Tests: 3 passed, 3 total"
```

Each test runs inside a transaction that it rolls back, so `app.db` stays as `setup` left it. Edit a query, such as adding a column to `queries/get_user.sql`, and the next `nub run test` runs the new statement with no `--clearCache`: the transformer's cache key is the query's module, so Jest's cache follows an edited file and a migrated schema alike.

## How Reylo is wired in

- In `package.json`:
  - In `dependencies`, `reylo` is the runtime each query module imports, and the transformer `reylo/jest`.
  - In `devDependencies`, `@reylosql/sqlite` and `typescript` (`7.1.0-beta`) are the SQLite engine that the type check and the transformer analyze each statement with, and the first TypeScript with content mappers.
  - The `test` script runs `node --experimental-vm-modules node_modules/jest/bin/jest.js`: Jest runs ES modules only behind that Node flag, and passing it to `node` directly works in every shell, with no `NODE_OPTIONS=` prefix or `cross-env`. The flag `--env-file-if-exists=.env` gives the tests `DATABASE_URL`; one set in the environment wins.
  - The `test:cjs` script runs Jest without the flag, with `jest.cjs.config.js`.
  - The `typecheck` script runs `tsc --noEmit --runExternalCode`, which lets `tsc` run the content mapper.
- In `tsconfig.json`, `contentMappers` hands `.sql` imports, and `.reylo.yaml`, the project file, to `reylo`. The `include` option covers `src`, where the tests sit beside the code they test, and `setup.ts`, so the type check covers the tests too; it leaves out `setup.sql`, a schema rather than one query, which the type check would report. The options `erasableSyntaxOnly` and `verbatimModuleSyntax` keep the TypeScript to what Node and Jest can run by stripping types, and `allowImportingTsExtensions` lets the files import each other as `./db.ts`, as Node needs.
- In `jest.config.js`, the ES module run, `transform` sends `.sql` files to `reylo/jest`, since Jest loads tests through its own runtime, which no preload reaches. The `extensionsToTreatAsEsm` option lists `.sql` beside `.ts`, so a query's module is an ES module like the test that imports it. No transform claims the `.ts` files, so Jest strips their types itself, through Node.
- The CommonJS run, `jest.cjs.config.js`, has the same transformer, which writes CommonJS when Jest asks for it; `babel-jest` for `.js` as well as `.ts`, because a CommonJS run must compile the tests' `import`s and Reylo's runtime, an ES module; and `transformIgnorePatterns`, which lets Babel compile `reylo` inside `node_modules`. Its Babel presets are preset-env, which compiles ES modules only when Jest asks for CommonJS (see Notes), and preset-typescript.
- The `queries/` directory holds one statement per `.sql` file. The module `src/blog.ts` imports them, and so does `src/blog.test.ts`, which also calls one directly. A CommonJS test written by hand reads a query as `require("../queries/get_user.sql").default`; Babel writes that for the TypeScript tests here.
- The `.env` file sets `DATABASE_URL=file:./app.db`, a path that starts at `package.json`. The type check and the transformer analyze each statement against it, and `src/db.ts` opens the same file.
- The `.vscode/` and `.zed/` directories hold the editor settings from [reylo.dev/#editors](https://reylo.dev/#editors), which run the project's TypeScript with content mappers on.

## In your own project

- Install the packages instead of linking them: `npm install reylo better-sqlite3`, then `npm install -D @reylosql/sqlite typescript@beta jest @jest/globals @types/better-sqlite3 @types/node`, and for the CommonJS run `npm install -D babel-jest @babel/core @babel/preset-env @babel/preset-typescript`.
- Keep the configuration of the mode you run, as `jest.config.js`. The ES module run needs no Babel. In a package without `"type": "module"`, write the configuration with `module.exports =`.
- Nothing else here exists only for the link. Linked, Reylo sits outside any `node_modules` (Jest sees its real path, `dist/reylo/`), so `transformIgnorePatterns` changes nothing in this repository; installed, it is what lets Babel compile Reylo, and its pattern matches both `node_modules/reylo/` (npm) and a store such as `node_modules/.pnpm/reylo@…/node_modules/reylo/` (pnpm, nub, Bun).

## Notes

- better-sqlite3, which the docs' SQLite examples use: version 13 carries prebuilt binaries inside the package, so it installs on Node 24 and 26 without a compiler or an install script. The built-in `node:sqlite` also works inside Jest's sandbox, without `db.transaction`.
- The TypeScript tests need no transform in the ES module run: Jest 30.5 strips the types of a `.ts` file that no transform claims, with Node's `module.stripTypeScriptTypes`. ts-jest does not run on TypeScript 7: it calls TypeScript's JavaScript API, which `typescript@7.1.0-beta` does not ship, and ts-jest 29.4 asks for `typescript <7`. So the CommonJS run compiles with babel-jest, the TypeScript setup in Jest's own docs. Neither checks types: `nub run typecheck` does.
- Under `--experimental-vm-modules`, Jest 30.5 on Node 24.9 or later loads an ES module that a CommonJS file requires as an ES module, and it fails with `ReferenceError: exports is not defined` when Babel has already compiled that module to CommonJS. The plugin `@babel/plugin-transform-modules-commonjs` always compiles, so the CommonJS run takes preset-env, which compiles only when Jest asks for CommonJS. nub starts every Node with that flag, so `nub run test:cjs` loads Reylo's runtime natively and `npm run test:cjs` compiles it; both pass.
- A statement the database refuses fails the test file that imports it, at the `.sql` file's line and column: `queries/get_user.sql:1:12: no such column: nmae`.
- Node prints `ExperimentalWarning: VM Modules is an experimental feature` on the ES module run.
- Jest 30.5 prints only the summary lines when it sees a coding agent's environment (`AI_AGENT`, `CLAUDECODE`); `--json` lists every test.
