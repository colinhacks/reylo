// The JavaScript half of a MySQL analysis, which this package carries with mysql2 inside it
// (scripts/build.ts bundles them into describe.js), and the analysis loads by its database
// (src/engine.ts, `describer`): a server-side prepare reports result columns with type codes, NOT NULL flags,
// charsets, display lengths and the table each is a column of, and the server's type guess per `?`
// parameter, without executing anything. The engine reads those as they are
// (wiki/mysql-sources.md); this side only carries them, and runs the engine's catalog queries.
import mysql from "mysql2/promise";
import { NotMysql, notMysql, Unreachable } from "../../src/connection.ts";
import type { MysqlDescribed } from "../../src/engine.ts";

export type { MysqlDescribed } from "../../src/engine.ts";
export { NotMysql, notMysql } from "../../src/connection.ts";

export interface MysqlDescriber {
  describe(sql: string): Promise<MysqlDescribed>;
  /** Rows as arrays of values, in column order. */
  values(sql: string, params: unknown[]): Promise<unknown[][]>;
  close(): Promise<void>;
}

/** How a connection is made: mysql2's, which the tests replace with a stand-in server. */
export const connector = { createConnection: (url: string): Promise<unknown> => mysql.createConnection(url) };

export async function openMysqlDescriber(url: string): Promise<MysqlDescriber> {
  // The server's answer to a statement carries a SQLSTATE; mysql2 marks `fatal` the errors that
  // end the connection (a refused socket, a handshake the server rejects, a lost connection).
  const rethrow = (e: unknown): never => {
    const err = e as { sqlState?: string; fatal?: boolean };
    throw err?.sqlState && !err.fatal ? e : new Unreachable(e);
  };
  // mysql2's default connectTimeout is already 10 s, the Postgres describer's limit.
  // Typed loosely, as the prepare's metadata is: mysql2's declarations leave out what it carries.
  const conn: any = await connector.createConnection(url).catch(rethrow);
  // A connection that drops while idle is reported as an event; unheard, it ends the process.
  conn.on("error", () => {});
  // One query per connection: a MariaDB server is refused before anything is analyzed on it. A
  // server that refuses the query (none known does) is taken for MySQL.
  let version: unknown;
  try {
    const [rows] = (await conn.query({ sql: "select version()", rowsAsArray: true })) as [unknown[][]];
    version = rows?.[0]?.[0];
  } catch (e) {
    const err = e as { sqlState?: string; fatal?: boolean };
    if (!err?.sqlState || err.fatal) {
      await conn.end().catch(() => {});
      throw new Unreachable(e);
    }
  }
  const other = notMysql(version);
  if (other) {
    await conn.end().catch(() => {});
    throw new NotMysql(other);
  }
  return {
    async describe(sql) {
      const prepared = await conn.prepare(sql).catch(rethrow);
      const info = prepared.statement ?? prepared;
      try {
        return {
          kind: "mysql",
          columns: (info.columns as any[]).map((c) => ({ name: c.name, type: c.columnType, flags: c.flags, charset: c.characterSet ?? 0, length: c.columnLength ?? 0, org_table: c.orgTable ?? "", org_name: c.orgName ?? "" })),
          params: (info.parameters as any[]).map((p) => ({ type: p.columnType, flags: p.flags ?? 0, charset: p.characterSet ?? 0 })),
        };
      } finally {
        await prepared.close();
      }
    },
    async values(sql, params) {
      const [rows] = await conn.query({ sql, values: params, rowsAsArray: true }).catch(rethrow);
      return rows as unknown[][];
    },
    close: () => conn.end(),
  };
}
