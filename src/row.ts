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
