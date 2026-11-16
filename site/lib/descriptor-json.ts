/**
 * A type descriptor as JSON, laid out as `reylo analyze` lays out its YAML (engine/cli/src/yaml.rs):
 * each type, and each object or list of plain values, on one line where it fits, everything else
 * one entry per line. The inference cards show their statements' parameters and results this way.
 */

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

const isMap = (v: Json): v is { [key: string]: Json } => typeof v === 'object' && v !== null && !Array.isArray(v);
const isContainer = (v: Json) => isMap(v) || Array.isArray(v);

/** The column a line stays within. */
const WIDTH = 72;

function inline(v: Json): string {
  if (Array.isArray(v)) return `[${v.map(inline).join(', ')}]`;
  if (isMap(v)) {
    const entries = Object.entries(v);
    return entries.length ? `{ ${entries.map(([k, x]) => `${JSON.stringify(k)}: ${inline(x)}`).join(', ')} }` : '{}';
  }
  return JSON.stringify(v);
}

/** Whether a value is written on one line: a type, or a container of plain values, that fits. */
function oneLine(v: Json, used: number): boolean {
  if (!isContainer(v)) return true;
  const flowing = isMap(v) ? typeof v.kind === 'string' || Object.values(v).every((x) => !isContainer(x)) : !v.some(isMap);
  return flowing && used + inline(v).length <= WIDTH;
}

function block(v: Json, indent: number, used: number): string {
  if (oneLine(v, used)) return inline(v);
  const pad = ' '.repeat(indent + 2);
  if (Array.isArray(v)) return `[\n${v.map((x) => pad + block(x, indent + 2, indent + 2)).join(',\n')}\n${' '.repeat(indent)}]`;
  const entries = Object.entries(v as { [key: string]: Json });
  return `{\n${entries.map(([k, x]) => `${pad}${JSON.stringify(k)}: ${block(x, indent + 2, indent + 4 + k.length + 2)}`).join(',\n')}\n${' '.repeat(indent)}}`;
}

/** `value` as JSON, laid out for reading. */
export function jsonDocument(value: Json): string {
  return block(value, 0, 0);
}
