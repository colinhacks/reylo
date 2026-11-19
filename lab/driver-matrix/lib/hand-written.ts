// A statement written by hand with Reylo's placeholders (`:id`), compiled as a build step compiles
// one, for the queries the scenarios define by hand, which the run time takes as written: the
// engine's lexer reads its placeholders (the `lex` request, through the one-shot
// `reylo engine <database> call`), and each is written as its database binds it, `$n` on Postgres,
// numbered once per name by `params`, and one `?` per placeholder on MySQL and SQLite. Erasable
// TypeScript, as the harness is, so Node and Bun run it as it is.
import { spawnSync } from "node:child_process";
import { EngineCore } from "../../../src/engine-core.ts";
import { nativeBinary } from "../../../src/native.ts";
import type { Dialect } from "./harness.ts";

/** What a query defined by hand passes beside its dialect: the statement as written and its names, and the statement its database receives, the name each value binds and each placeholder's place in it. */
export type HandWritten = { sql: string; params: string[]; text: string; order: string[]; marks: [number, number][] };

const kept = new Map<string, HandWritten>();

export function handWritten(sql: string, dialect: Dialect, params?: readonly string[]): HandWritten {
  const key = JSON.stringify([sql, dialect, params]);
  const known = kept.get(key);
  if (known) return known;
  const binary = nativeBinary();
  const engine = new EngineCore((request) => {
    const run = spawnSync(binary, ["engine", dialect, "call"], { input: request, encoding: "utf8" });
    if (run.status !== 0) throw new Error(`the engine ${binary} exited with ${run.status ?? run.signal}: ${run.stderr}`);
    return run.stdout;
  });
  const lexed = engine.lex(sql, dialect);
  const names = params ?? lexed.params;
  // The engine's offsets are UTF-8 bytes.
  const bytes = Buffer.from(sql, "utf8");
  const at = (offset: number) => bytes.subarray(0, offset).toString("utf8").length;
  let text = "";
  let last = 0;
  const order: string[] = [];
  const marks: [number, number][] = [];
  for (const p of lexed.placeholders) {
    if (dialect === "postgres" && !names.includes(p.name)) throw new Error(`${JSON.stringify(sql)}: its placeholder for ${p.name} is not one of ${JSON.stringify(names)}`);
    text += sql.slice(last, at(p.start));
    const written = dialect === "postgres" ? `$${names.indexOf(p.name) + 1}` : "?";
    marks.push([text.length, text.length + written.length]);
    text += written;
    order.push(p.name);
    last = at(p.end);
  }
  text += sql.slice(last);
  const compiled = { sql, params: [...names], text, order: dialect === "postgres" ? [...names] : order, marks };
  kept.set(key, compiled);
  return compiled;
}
