// The TypeScript type of a typed JSON parameter, from the schema in its cast
// (`:users::jsonb('{ ... }')`, wiki/typed-json-params.md). The loader types such a parameter
// `unknown` until typed JSON parameters are built; scripts/capture-types.ts puts this type in its
// place, so the docs show what the build will give.
//
// The schema is JSON Schema 2020-12 with its own type names: `type` is
// `string`, `number`, `integer`, `boolean`, `null`, `object` or `array`, or a list of them; a key is
// optional unless `required` lists it; every schema is an object, never a bare type name; `enum` and
// `const` narrow a type, and `oneOf`, `anyOf` and `allOf` combine schemas. A database type name is an
// error: database types belong to the statement's column list and to `row` and `rows` descriptors
// (wiki/structured-params.md).

export type Dialect = 'postgres' | 'mysql' | 'sqlite' | 'duckdb';
type Schema = string | boolean | { [key: string]: unknown };

const LEAVES: Record<string, string> = { string: 'string', number: 'number', integer: 'number', boolean: 'boolean', null: 'null' };

/** A JSON Schema type name, other than `object` and `array`, as TypeScript. */
function leaf(name: string, _dialect: Dialect, at: string): string {
  const type = LEAVES[name];
  if (type === undefined) throw new Error(`${at}: "${name}" is not one of JSON Schema's types (${[...Object.keys(LEAVES), 'object', 'array'].join(', ')}); a database type goes in the statement's column list`);
  return type;
}

const ident = (name: string) => (/^[A-Za-z_$][\w$]*$/.test(name) ? name : JSON.stringify(name));
/** A type as an array's element or an intersection's member: in parentheses when it is a union or an intersection. */
const parens = (type: string) => (topLevel(type, ' | ').length > 1 || topLevel(type, ' & ').length > 1 ? `(${type})` : type);
const union = (types: string[]) => {
  const parts = [...new Set(types.flatMap((t) => topLevel(t, ' | ')))];
  if (parts.includes('unknown')) return 'unknown';
  return parts.join(' | ') || 'never';
};
/** The parts of a type around a separator at its top level, never inside braces, brackets or a string. */
function topLevel(type: string, separator: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < type.length; i++) {
    const c = type[i];
    if (c === '{' || c === '[' || c === '(' || c === '<') depth++;
    else if (c === '}' || c === ']' || c === ')' || c === '>') depth--;
    else if (c === '"') i = skipString(type, i);
    else if (depth === 0 && type.startsWith(separator, i)) {
      parts.push(type.slice(start, i));
      start = i + separator.length;
      i += separator.length - 1;
    }
  }
  parts.push(type.slice(start));
  return parts.map((p) => p.trim()).filter(Boolean);
}
function skipString(text: string, i: number): number {
  for (let j = i + 1; j < text.length; j++) {
    if (text[j] === '\\') j++;
    else if (text[j] === '"') return j;
  }
  return text.length;
}

/**
 * The TypeScript type of a value the schema accepts. `dialect` reads its type names; `at` names
 * the place for an error (the parameter).
 */
export function schemaType(schema: Schema, dialect: Dialect, at = 'the schema'): string {
  const type = (s: unknown, where: string): string => {
    if (s === true) return 'unknown';
    if (s === false) return 'never';
    if (s === null || typeof s !== 'object' || Array.isArray(s)) throw new Error(`${where}: a schema is an object, as in { "type": "text" }, not ${JSON.stringify(s)}`);
    const o = s as Record<string, unknown>;
    if (Object.keys(o).length === 0) return 'unknown';
    if ('const' in o) return JSON.stringify(o.const);
    if (Array.isArray(o.enum)) return union(o.enum.map((v) => JSON.stringify(v)));
    const alternatives = (['oneOf', 'anyOf'] as const).find((k) => Array.isArray(o[k]));
    const own = Object.keys(o).some((k) => ['type', 'properties', 'items', 'prefixItems', 'additionalProperties', 'required'].includes(k));
    if (alternatives) {
      const members = union((o[alternatives] as unknown[]).map((m, i) => type(m, `${where}.${alternatives}[${i}]`)));
      if (!own) return members;
      const { [alternatives]: _, ...base } = o;
      return `${parens(type(base, where))} & (${members})`;
    }
    if (Array.isArray(o.allOf)) return (o.allOf as unknown[]).map((m, i) => parens(type(m, `${where}.allOf[${i}]`))).join(' & ');
    const types = typeof o.type === 'string' ? [o.type] : Array.isArray(o.type) ? (o.type as unknown[]) : o.properties || o.additionalProperties !== undefined ? ['object'] : o.items || o.prefixItems ? ['array'] : [];
    if (!types.length) throw new Error(`${where}: a schema names its "type"`);
    return union(
      types.map((t) => {
        if (typeof t !== 'string') throw new Error(`${where}: "type" holds ${JSON.stringify(t)}, not a type name`);
        if (t === 'object') return object(o, where);
        if (t === 'array') return array(o, where);
        return leaf(t, dialect, where);
      }),
    );
  };
  const object = (o: Record<string, unknown>, where: string): string => {
    const props = (o.properties ?? {}) as Record<string, unknown>;
    if (typeof props !== 'object' || Array.isArray(props)) throw new Error(`${where}: "properties" is an object of schemas`);
    // A key is optional unless `required` lists it, as in JSON Schema.
    const required = new Set(Array.isArray(o.required) ? (o.required as string[]) : []);
    for (const name of required) if (!(name in props)) throw new Error(`${where}: "required" lists "${name}", which "properties" does not name`);
    const fields = Object.entries(props).map(([name, value]) => `${ident(name)}${required.has(name) ? '' : '?'}: ${type(value, `${where}.${name}`)}`);
    const extra = o.additionalProperties;
    const rest = extra === undefined || extra === false ? undefined : extra === true ? 'unknown' : type(extra, `${where}.additionalProperties`);
    if (!fields.length) return rest === undefined ? (extra === false ? 'Record<never, never>' : 'Record<string, unknown>') : `Record<string, ${rest}>`;
    const shape = `{ ${fields.join('; ')} }`;
    return rest === undefined ? shape : `${shape} & Record<string, ${rest}>`;
  };
  const array = (o: Record<string, unknown>, where: string): string => {
    if (Array.isArray(o.prefixItems)) return `[${(o.prefixItems as unknown[]).map((m, i) => type(m, `${where}[${i}]`)).join(', ')}]`;
    if (o.items === undefined) return 'unknown[]';
    return `${parens(type(o.items, `${where}[]`))}[]`;
  };
  return type(schema, at);
}
