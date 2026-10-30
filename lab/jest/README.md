# Lab: Jest and Vitest

This lab answers whether `node --import reylo/register` makes a `.sql` import work under Jest. It does not, in a CommonJS run or an ESM run. The lab holds the experiment, the `reylo/jest` transformer runs that do work, and one Vitest run through `reylo/vite`. Jest is 30.5.2 on Node 26.10.

```sh
cd lab/jest
nub install && ln -sfn ../../.. node_modules/reylo   # nub install removes the symlink; link it again after every install
mkdir -p node_modules/@reylosql && ln -sfn ../../../../packages/sqlite node_modules/@reylosql/sqlite   # the SQLite engine, likewise
node setup.mjs                                      # lab.db; .reylo.yaml points at it with file:./lab.db
nub run test:cjs                                    # reylo/jest, CommonJS run: query.test.cjs
nub run test:esm                                    # reylo/jest, --experimental-vm-modules: query.test.cjs and query.test.mjs
nub run test:vitest                                 # reylo/vite: query.test.mjs
```

Watchman is broken on the machine the lab was written on, and Jest then exits 0 without running a test. Every config sets `watchman: false`, and a run without a config needs `--no-watchman`.

## The preload under Jest

The preload reaches every Jest process, and Jest never asks Node's loader for a test's modules, so no preload types a test's `.sql` import (`tests/jest.test.ts` checks it).

A `SyntaxError: Unexpected identifier 'id'` ends `query.test.cjs` and `query.test.mjs` under every preload form: `NODE_OPTIONS`, `node --import`, `nubx jest`, workers, `--runInBand`, `--experimental-vm-modules`, and `extensionsToTreatAsEsm: [".sql"]`. The cause is in `jest-runtime/build/index.js` (30.5.2):

- A CommonJS module is read with `fs.readFileSync` (`FileCache.readFile`), passed through the `transform` entries, and compiled with `vm.compileFunction` in the test's context (`ModuleExecutor.exec` and `compile`). Node's `require` is never called.
- An ES module is built as `new vm.SourceTextModule(code, { context })` from the same transform output (`EsmLoader`). Node's ESM loader is never called either.
- Inside the sandbox, Jest replaces `module.register` and `module.registerHooks` with a function that throws. Its message says the hooks would attach to the loader running Jest itself, and points to `transform` instead. Jest also replaces `process.getBuiltinModule`.

## `reylo/jest`

The transformer (`src/jest.ts`) answers Jest's synchronous `process` through the worker bridge that `reylo/register` uses (`src/bridge.ts`). It returns ESM when Jest loads the file as an ES module and CommonJS otherwise. Its cache key is the module itself, so Jest's transform cache follows a migration.

Reylo's runtime (`import { defineQuery } from "reylo"`) is an ES module. A run without `--experimental-vm-modules` cannot `require` an ES module, so that run also needs Reylo compiled to CommonJS.

| Config | Run | Result |
| --- | --- | --- |
| `jest.esm.config.cjs` | `--experimental-vm-modules`, workers and `--runInBand` | both tests pass |
| the same, without `reylo/jest` | `--experimental-vm-modules`, with and without `--import reylo/register` | `SyntaxError` in both tests |
| `reylo/jest` without `extensionsToTreatAsEsm` | `--experimental-vm-modules` | `query.test.cjs` passes; `query.test.mjs` gets `module.exports` as its default import, so `query.cardinality` is `undefined` |
| `jest.cjs.config.cjs` (babel-jest compiles Reylo) | no flag | `query.test.cjs` passes |
| the same, without `reylo/jest` | no flag, with and without `--import reylo/register` | `SyntaxError: Unexpected identifier 'id'` |
| `reylo/jest` alone | no flag | `Must use import to load ES Module: .../src/index.ts` |
| `reylo/jest` with ESM output forced | no flag | `Must use import to load ES Module: .../query.sql` |
| `reylo/jest` without its cache key | two runs, one cache directory, a unique index added between them | the second run keeps `many`; with the key it reads `maybeOne` |

A test that expects a wrong email fails in each passing config. The built package (`nub run build`, `dist/` copied into a project's `node_modules/reylo`) gives the same results. The ESM config passes both tests. The CommonJS config passes with `transformIgnorePatterns: ["/node_modules/(?!reylo/)"]` and fails with `Must use import to load ES Module: .../node_modules/reylo/src/index.js` without it.

## Vitest

Vitest needs only `reylo/vite` (`vitest.config.mjs`). Without the plugin, Vite fails to parse `query.sql`, and a wrong expectation fails the run.

Vitest keeps a transformed module across runs on disk (`fsModuleCache`, on by default from Vitest 5.0.5; this lab's 5.0.3 has it off), keyed by its file's text, and in watch mode keeps that key until the file changes. A migration changes a `.sql` module and not its file, so the plugin's `configureVitest` has Vitest keep no `.sql` module. `tests/jest.test.ts` checks this in a project outside the repository with the cache on. Across runs, with a unique index added between them, the second run reads `maybeOne`. Under `vitest --watch`, the run after `reylo generate` writes the new analysis reads it too. Without the hook, both runs keep `many`. Vitest 5.0.5 behaves the same with no cache setting in its config.

`tests/jest.test.ts` at the root runs the preload control, both Jest configs and Vitest, and skips when the lab is not installed.
