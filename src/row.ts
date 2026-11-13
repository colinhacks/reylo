// A result row as an object: one property per output column, in the statement's order, each
// column's value already decoded. A build step writes the builder into the module as an object
// literal (`rowSource`), so that every row of a query has one shape, made in one allocation,
// with no code compiled at run time; a query defined by hand builds the same object in a loop
// (`rowOf`). Either way a column named `__proto__` is an own property like any other, never the
// row's prototype, and of two columns with one name the row holds the last one's value, at the
// first one's position, as an assignment of each in turn would leave it.

/** Builds one row from its values, in the order of the columns. */
export type RowBuilder = (values: readonly unknown[]) => Record<string, unknown>;

/** The source of a module's row builder for these column names: an arrow function returning an object literal. */
export function rowSource(names: readonly string[]): string {
  // A computed key defines an own property; a plain `"__proto__":` would set the prototype.
  const props = names.map((name, i) => `${name === "__proto__" ? `["__proto__"]` : JSON.stringify(name)}: v[${i}]`);
  return `(v) => ({ ${props.join(", ")} })`;
}

/** The row builder for these column names, without a module's own: the same object, built in a loop. */
export function rowOf(names: readonly string[]): RowBuilder {
  const n = names.length;
  if (!names.includes("__proto__")) {
    return (v) => {
      const row: Record<string, unknown> = {};
      for (let i = 0; i < n; i++) row[names[i]!] = v[i];
      return row;
    };
  }
  return (v) => {
    const row: Record<string, unknown> = {};
    for (let i = 0; i < n; i++) Object.defineProperty(row, names[i]!, { value: v[i], writable: true, enumerable: true, configurable: true });
    return row;
  };
}

/** The last column of each name: a row holds one value per name, the last column's, so what is done to a value by its name is that column's to say. */
export function lastOfEach<T extends readonly [name: string, ...rest: unknown[]]>(columns: readonly T[]): T[] {
  const last = new Map<string, number>();
  columns.forEach(([name], i) => last.set(name, i));
  return columns.filter(([name], i) => last.get(name) === i);
}

/**
 * The names a result's values go under, in the result's order, for a driver that reads rows by
 * position. They are the module's where the result is still its columns: a driver may name a column
 * with no alias otherwise than the module does (one that writes a value into the statement's text
 * names the column after the value, which may be another column's name), and the module's name
 * for such a column is nowhere in the result. Under `select *` a migration breaks that, and the
 * names are then the result's own: one that adds or drops a column changes the width, and one
 * that moves a column, or drops one and adds another, leaves a declared name at another position.
 */
export function namesInResult(declared: readonly string[], result: readonly string[]): readonly string[] {
  const n = declared.length;
  if (n !== result.length) return result;
  let i = 0;
  while (i < n && declared[i] === result[i]) i++;
  if (i === n) return declared;
  const theirs = new Set(result);
  for (; i < n; i++) if (declared[i] !== result[i] && theirs.has(declared[i]!)) return result;
  return declared;
}
