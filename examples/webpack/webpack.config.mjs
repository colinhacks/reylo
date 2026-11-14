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
      // TypeScript 7's package has no JavaScript API, which ts-loader needs: swc strips the types.
      {
        test: /\.ts$/,
        use: { loader: "swc-loader", options: { jsc: { parser: { syntax: "typescript" }, target: "es2024" } } },
      },
    ],
  },
};
