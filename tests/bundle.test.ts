// What an app's bundle holds of the run time: the half of the dialect its queries are written for,
// and no other dialect's drivers or error codes (src/execute.ts, `Runtime`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { build } from "esbuild";
import { DEFINE } from "../src/loader.ts";
import type { Dialect } from "../src/execute.ts";
import { ROOT } from "./helpers.ts";

/** Each dialect's drivers and error codes. MySQL's include Bun.sql's reader of rows, which Postgres's module holds (`bunRows`). */
const OWN: Record<Dialect, string[]> = {
  postgres: ["src/decode.ts", "src/error-codes-postgres.ts"],
  mysql: ["src/decode-mysql.ts", "src/decode.ts", "src/error-codes-mysql.ts"],
  sqlite: ["src/decode-sqlite.ts", "src/error-codes-sqlite.ts"],
  duckdb: ["src/decode-duckdb.ts"],
};

test("a bundle of a query holds its dialect's drivers and error codes, and no other dialect's", async () => {
  for (const dialect of Object.keys(OWN) as Dialect[]) {
    // The module a build step writes, as src/loader.ts writes it.
    const contents = `import { ${DEFINE[dialect]} } from "reylo";\nexport default ${DEFINE[dialect]}({ sql: "select 1 as n", params: [], cardinality: "one", columns: [["n", "int"]] });\n`;
    const { metafile } = await build({ stdin: { contents, resolveDir: ROOT }, bundle: true, minify: true, platform: "node", format: "esm", write: false, metafile: true, alias: { reylo: join(ROOT, "src", "index.ts") }, logLevel: "silent" });
    const held = Object.entries(Object.values(metafile.outputs)[0]!.inputs).flatMap(([path, input]) => (input.bytesInOutput > 0 && /^src\/(decode|error-codes)/.test(path) ? [path] : []));
    assert.deepEqual(held.sort(), OWN[dialect].sort(), dialect);
  }
});
