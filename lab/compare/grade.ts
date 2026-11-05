// Turns each tool's answers into the comparison's marks: a mark is computed from what the tool answered and nothing else.
import { DBS, CARDINALITY, EXPRESSIONS, FROM_THE_DATABASE, FROM_THE_DATABASE_KINDS, JSON_SCHEMA, JSON_SHAPES, OUTER_JOINS, PARAMETER_NULLABILITY, PARAMETER_TYPES, sqlFor, type Db, type Probe } from "./probes.ts";
import type { Answer, TypedValue } from "./tools/lib.ts";

export type State = "y" | "p" | "n";
export interface ProbeResult { db: Db; probe: string; sql: string; ok: boolean; problems: string[] }
export interface Cell { state: State; right: number; total: number; note?: string; results: ProbeResult[] }
type Answers = Record<string, Answer>; // keyed `${db}:${group}:${probe}`

const CARD_WORDS = { one: "exactly one row", maybeOne: "at most one row", many: "many rows", none: "no rows" };
const key = (db: Db, group: string, p: Probe) => `${db}:${group}:${p.id}`;

function checkColumns(p: Probe, a: Answer, checkNull: boolean, db: Db): string[] {
  const out: string[] = [];
  (p.columns ?? []).forEach((want, i) => {
    const got = a.columns?.find((c) => c.name.toLowerCase() === want.name.toLowerCase()) ?? a.columns?.[i];
    if (!got) return void out.push(`\`${want.name}\` is missing`);
    if (checkNull && want.nullable !== undefined && !want.anyNullOn?.includes(db) && got.nullable !== want.nullable) out.push(`\`${want.name}\` is typed \`${got.type}\`, but ${want.nullable ? "it can be null" : "it is never null"}`);
    if (want.shape && !want.shape.every((f) => got.fields?.includes(f))) out.push(`\`${want.name}\` is typed \`${got.type}\`, without its fields`);
  });
  return out;
}
function checkParams(p: Probe, a: Answer, db: Db): string[] {
  const out: string[] = [];
  (p.params ?? []).forEach((want, i) => {
    if (!want.kinds && want.nullable === undefined && !want.shape) return;
    const got: TypedValue | undefined = a.params?.[i];
    if (!got) return void out.push(`parameter ${i + 1} has no type`);
    const takes = want.kinds?.[0] === "num" ? "a number" : want.kinds?.[0] === "time" ? "a timestamp" : "text";
    if (want.kinds && !want.kinds.includes(got.kind)) out.push(got.wrongValuePasses ? `a ${got.wrongValuePasses} value passes for parameter ${i + 1}, which takes ${takes}` : `parameter ${i + 1} is typed \`${got.type}\`, but it takes ${takes}`);
    if (want.nullable !== undefined && !want.anyNullOn?.includes(db) && got.nullable !== want.nullable) out.push(got.checksValues ? (want.nullable ? `a null value is refused for parameter ${i + 1}, which takes null` : `a null value passes for parameter ${i + 1}, which takes no null`) : `parameter ${i + 1} is typed \`${got.type}\`, but it ${want.nullable ? "takes null" : "takes no null"}`);
    if (want.shape && !want.shape.every((f) => got.fields?.includes(f))) out.push(`parameter ${i + 1} is typed \`${got.type}\`, without its fields`);
  });
  return out;
}
// Answers for one group of probes on one database: right when `check` finds no problem.
function run(answers: Answers, dbs: Db[], group: string, probes: Probe[], check: (p: Probe, a: Answer, db: Db) => string[]): ProbeResult[] {
  const out: ProbeResult[] = [];
  for (const db of dbs) for (const p of probes) {
    const sql = sqlFor(p, db);
    if (!sql) continue;
    const a = answers[key(db, group, p)];
    if (!a) continue;
    const problems = a.ok ? check(p, a, db) : [`it reports an error: ${a.error ?? "unknown"}`];
    out.push({ db, probe: p.id, sql, ok: problems.length === 0, problems });
  }
  return out;
}
function cell(results: ProbeResult[], constant = false, note?: string): Cell {
  const right = results.filter((r) => r.ok).length;
  const state: State = results.length === 0 || right === 0 || constant ? "n" : right === results.length ? "y" : "p";
  return { state, right, total: results.length, note, results };
}
// True when every answer on a database is the same whatever the statement, the sign of a rule that reads nothing from the query.
function constantOn(values: boolean[]) { return values.length > 1 && values.every((v) => v === values[0]); }

export function gradeTool(answers: Answers, dbs: Db[], facts: { wrote: string[]; connects: "yes" | "opt-in" | "no"; cardinalityNote?: string; noConnectNote?: string; optInNote?: string; withoutGenerating?: string | null; supportNotes?: Partial<Record<string, string>> }) {
  const cells: Record<string, Cell> = {};

  const none = facts.wrote.length === 0;
  const modified = facts.wrote.every((l) => l.startsWith("M "));
  cells["no-codegen"] = { state: none ? "y" : modified ? "p" : "n", right: none ? 1 : 0, total: 1, note: none ? "Nothing to commit after a run." : `A run leaves ${modified ? "changed source" : "new files"} to commit: ${facts.wrote.map((l) => `\`${l.replace(/^\S+\s+/, "")}\``).slice(0, 3).join(", ")}.`, results: [] };

  const live = run(answers, dbs.includes("postgres") ? ["postgres"] : [], "from-the-database", FROM_THE_DATABASE, (p, a) => {
    const c = a.columns?.[0];
    if (!c) return ["it gives no type"];
    return FROM_THE_DATABASE_KINDS[p.id].includes(c.kind) ? [] : [`it types the result \`${c.type}\``];
  });
  const liveRight = live.filter((r) => r.ok).length;
  if (!none && facts.withoutGenerating) cells["no-codegen"] = { state: "p", right: 0, total: 1, note: facts.withoutGenerating, results: [] };
  cells["from-the-database"] = { state: facts.connects === "yes" && liveRight === live.length ? "y" : facts.connects === "no" ? "n" : "p", right: liveRight, total: live.length, note: facts.connects === "no" ? facts.noConnectNote ?? "It never connects to a database." : facts.connects === "opt-in" ? facts.optInNote : liveRight < live.length ? "It connects, but types some expressions itself." : undefined, results: live };

  const joins = run(answers, dbs, "outer-joins", OUTER_JOINS, (p, a, db) => checkColumns(p, a, true, db));
  cells["outer-joins"] = cell(joins);

  const exprResults = run(answers, dbs, "expressions", EXPRESSIONS, (p, a, db) => checkColumns(p, a, true, db));
  const exprConstant = dbs.every((db) => constantOn(EXPRESSIONS.filter((p) => sqlFor(p, db)).map((p) => !!answers[key(db, "expressions", p)]?.columns?.[0]?.nullable)));
  cells["expression-nullability"] = cell(exprResults, exprConstant, exprConstant ? "It gives every expression the same answer." : undefined);

  cells["parameter-types"] = cell(run(answers, dbs, "parameter-types", PARAMETER_TYPES, checkParams));

  const nullResults = run(answers, dbs, "parameter-nullability", PARAMETER_NULLABILITY, checkParams);
  const nullConstant = dbs.every((db) => constantOn(PARAMETER_NULLABILITY.filter((p) => sqlFor(p, db)).flatMap((p) => (answers[key(db, "parameter-nullability", p)]?.params ?? []).map((v) => v.nullable))));
  cells["parameter-nullability"] = cell(nullResults, nullConstant, nullConstant ? "It gives every parameter the same answer." : undefined);

  const cardResults = run(answers, dbs, "cardinality", CARDINALITY, (p, a) => {
    if (a.cardinality === null) return ["it does not infer one"];
    return a.cardinality === p.cardinality ? [] : [`it says ${CARD_WORDS[a.cardinality]}, but this returns ${CARD_WORDS[p.cardinality!]}`];
  });
  const noCard = CARDINALITY.every((p) => dbs.every((db) => { const a = answers[key(db, "cardinality", p)]; return !a || a.cardinality === null; }));
  cells["cardinality"] = cell(cardResults, noCard, noCard ? facts.cardinalityNote : undefined);

  cells["json-shapes"] = cell(run(answers, dbs, "json-shapes", JSON_SHAPES, (p, a, db) => checkColumns(p, a, false, db)));

  const schema = run(answers, dbs.includes("postgres") ? ["postgres"] : [], "json-schema", JSON_SCHEMA, (p, a, db) => [...checkColumns(p, a, false, db), ...checkParams(p, a, db)]);
  cells["json-schema"] = cell(schema);

  for (const db of DBS) {
    const a = answers[`${db}:smoke:smoke`];
    const ok = !!a?.ok && (a.columns?.length ?? 0) === 2;
    cells[db] = { state: ok ? "y" : "n", right: ok ? 1 : 0, total: 1, note: ok ? undefined : facts.supportNotes?.[db] ?? (a ? a.error : `It is not available for ${db === "mysql" ? "MySQL" : db === "sqlite" ? "SQLite" : "Postgres"}.`), results: [] };
  }
  return cells;
}
