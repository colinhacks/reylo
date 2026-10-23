// The ways a describer fails before it describes anything, which the analysis tells apart from a
// statement's own error. Each engine package bundles its database's describer apart from the rest
// of the type check (src/tooling.ts), so each bundle has a copy of these classes of its own, and
// `instanceof` reads a mark from the global symbol registry instead of comparing classes.

const UNREACHABLE = Symbol.for("reylo.unreachable");
const NOT_MYSQL = Symbol.for("reylo.not-mysql");

/**
 * The database cannot be reached, as opposed to the server answering a statement with an error.
 * It ends the analysis at once instead of becoming an answer: asking again cannot succeed, and a
 * driver that backs off between attempts (postgres.js waits up to 20 s) would hold the transform
 * for minutes.
 */
export class Unreachable extends Error {
  readonly [UNREACHABLE] = true;
  constructor(cause: unknown) {
    super(reason(cause), { cause });
  }
  static [Symbol.hasInstance](value: unknown): boolean {
    return (value as { [UNREACHABLE]?: unknown } | null)?.[UNREACHABLE] === true;
  }
}

/** A driver error as one line: a refused connection is an AggregateError with no message of its own. */
export function reason(e: unknown): string {
  const err = e as { message?: string; code?: string; errors?: { message?: string }[] } | undefined;
  return err?.message || err?.errors?.map((x) => x.message).filter(Boolean).join("; ") || err?.code || String(e);
}

/**
 * The server the type check was pointed at is not MySQL. MariaDB answers a prepare with every
 * parameter typed NULL and a JSON column as LONGTEXT, so its statements would be typed wrongly
 * (wiki/dialects.md): the type check refuses it before any statement is analyzed.
 */
export class NotMysql extends Error {
  readonly [NOT_MYSQL] = true;
  readonly server: string;
  constructor(server: string) {
    super(`the server is ${server}, not MySQL`);
    this.server = server;
  }
  static [Symbol.hasInstance](value: unknown): boolean {
    return (value as { [NOT_MYSQL]?: unknown } | null)?.[NOT_MYSQL] === true;
  }
}

/** The server's name and version, when it is not MySQL: from version(), "11.8.2-MariaDB-ubu2404" on MariaDB. */
export function notMysql(version: unknown): string | undefined {
  const v = String(version ?? "");
  const m = /^(?:5\.5\.5-)?(\d+(?:\.\d+)*).*mariadb/i.exec(v);
  if (m) return `MariaDB ${m[1]}`;
  return /mariadb/i.test(v) ? "MariaDB" : undefined;
}
