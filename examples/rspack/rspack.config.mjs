import { rspack } from "@rspack/core";

// A server bundle: one ES module, dist/server.mjs, with its dependencies inside, run with
// `node dist/server.mjs` and no preload.
export default {
  mode: "production",
  target: "node24",
  entry: "./src/server.ts",
  output: { filename: "server.mjs", module: true, clean: true },
  module: {
    rules: [
      // Reylo: each .sql import becomes a module that runs the statement. The loader analyzes
      // the statement against the database DATABASE_URL names (or reads reylo/.cache/) at build time.
      { test: /\.sql$/, use: "reylo/webpack" },
      {
        test: /\.ts$/,
        loader: "builtin:swc-loader",
        options: { jsc: { parser: { syntax: "typescript" } } },
      },
    ],
  },
  // pg loads its optional native binding, pg-native, only for `pg.native`, which this app never
  // reads: leave it out of the bundle instead of warning that it is missing.
  plugins: [new rspack.IgnorePlugin({ resourceRegExp: /^pg-native$/ })],
};
