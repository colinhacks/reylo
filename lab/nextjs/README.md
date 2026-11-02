# Next.js lab

A Next.js 16.3 App Router app that imports `get_user.sql` in a route handler (`app/api/user/route.ts`), a server component (`app/page.tsx`) and an edge route handler (`app/api/edge-user/route.ts`, with a stub client, because postgres.js needs TCP sockets the edge runtime does not have). `types.ts` pins the inferred row and parameter types exactly.

```sh
nub install
ln -sfn ../../.. node_modules/reylo
(cd ../.. && cargo build --release -p reylo)   # the engine: the link finds the checkout's target/release/reylo, or REYLO_ENGINE_PATH names one
echo 'DATABASE_URL=postgres://localhost:5432/postgres' > .env   # the lab/basic tables
nub run typecheck        # next typegen, then tsc --noEmit --runExternalCode
nub run dev              # Turbopack; nub run dev:webpack for webpack
nub run build && nub run start   # next build, whose type check runs Reylo's; build:webpack for webpack
curl 'http://localhost:3000/api/user?id=1'
```

- `next.config.ts` wraps the config in `withReylo` from `reylo/next`, which registers the loader for `*.sql` in `turbopack.rules`, as `reylo/turbopack`, and in the `webpack` hook, as `reylo/webpack`, on the server only: a client component that imports a `.sql` file fails the build instead of shipping the statement to the browser.
- `next build` type-checks by running the project's `tsc` without `--runExternalCode`, so the content mapper cannot run there and every `.sql` import is TS2307 (plus TS100024 on the tsconfig, TS18068 from the nightly of 2026-09-23). `withReylo` sets `typescript.ignoreBuildErrors` to turn that step off, and runs `tsc --noEmit --runExternalCode` in `compiler.runAfterProductionCompile`, after the compile and before the prerender, which also checks the route types Next generates in `.next/types` (`wiki/nextjs.md`). The `typecheck` script is the same check without a build.
- `turbopack.root` is the repository root because `node_modules/reylo` is a symlink to it and Turbopack resolves nothing outside its root; an app that installs Reylo keeps the default.
- `allowImportingTsExtensions` is needed because the symlink reaches Reylo's TypeScript sources, whose imports end in `.ts`. The published package is compiled (`nub run build` writes it to `dist/`), so an app that installs it needs neither this setting nor the `turbopack.root` above; that was checked with the lab copied out of the repository and the built tarball installed.
- `/api/user` also returns `rewritten`, the statement as the engine rewrote it for the database, which the loader puts in every module.
- The routes answer through `json.ts`, which writes a BigInt (Reylo's 64-bit integer, `big`) as its text, since `JSON.stringify` and `Response.json` refuse one; it imports no driver, so the edge route can use it.
- In Zed, `.zed/settings.json` runs this project's `node_modules/typescript` as the language server with `runExternalCode` on; it needs the "TypeScript Language Server" extension (id `tsgo`) installed and the folder trusted, and Zed shows no diagnostics for the `.sql` files themselves, only types that follow each saved `.sql` file.
