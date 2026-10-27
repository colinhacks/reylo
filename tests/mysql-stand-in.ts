// A MySQL server stand-in for the type check's tests: enough of MySQL's client/server protocol for
// the engine's own driver to log in, run text queries and quit, over a real socket. It answers
// `select version()` with the version a test sets and every other query with one row, refuses a
// password it does not take as MySQL does (error 1045, at the login), and closes the connection on a
// prepare, so an analysis on it fails as on a server that went away. Each password tried is recorded
// by its name among the ones a test may try, since the login carries only its scramble.
import { createHash, randomBytes } from "node:crypto";
import { createServer, type AddressInfo, type Socket } from "node:net";

export type MysqlStandIn = {
  port: number;
  /** Every query and statement the stand-in was sent, in order. */
  queries: string[];
  /** Every password a login tried, by its name in `passwords` ("?" for another). */
  tried: string[];
  /** How many logged-in connections are open. */
  open(): number;
  /** Whether a connection said goodbye (COM_QUIT). */
  ended(): boolean;
  close(): Promise<void>;
};

const CLIENT = { LONG_PASSWORD: 0x1, FOUND_ROWS: 0x2, LONG_FLAG: 0x4, CONNECT_WITH_DB: 0x8, PROTOCOL_41: 0x200, TRANSACTIONS: 0x2000, SECURE_CONNECTION: 0x8000, MULTI_RESULTS: 0x20000, PLUGIN_AUTH: 0x80000, PLUGIN_AUTH_LENENC_CLIENT_DATA: 0x200000 };
const CAPABILITIES = Object.values(CLIENT).reduce((a, b) => a | b, 0);

const sha1 = (...parts: Buffer[]) => createHash("sha1").update(Buffer.concat(parts)).digest();

/** mysql_native_password's answer to a scramble: SHA1(password) XOR SHA1(scramble + SHA1(SHA1(password))). */
function scrambled(password: string, salt: Buffer): Buffer {
  if (password === "") return Buffer.alloc(0);
  const once = sha1(Buffer.from(password));
  const mask = sha1(salt, sha1(once));
  return Buffer.from(once.map((b, i) => b ^ mask[i]!));
}

function lenenc(n: number): Buffer {
  if (n < 251) return Buffer.from([n]);
  const b = Buffer.alloc(3);
  b[0] = 0xfc;
  b.writeUInt16LE(n, 1);
  return b;
}
const str = (s: string) => Buffer.concat([lenenc(Buffer.byteLength(s)), Buffer.from(s)]);
const u16 = (n: number) => Buffer.from([n & 0xff, n >> 8]);
const OK = Buffer.from([0x00, 0, 0, 2, 0, 0, 0]);
const EOF = Buffer.from([0xfe, 0, 0, 2, 0]);
const err = (code: number, state: string, message: string) => Buffer.concat([Buffer.from([0xff]), u16(code), Buffer.from(`#${state}${message}`)]);

/**
 * A stand-in on a free port. `version` is what `select version()` answers; `accepts` names the
 * password it takes now, or every password where it returns undefined; `passwords` are the names
 * `tried` records a login by.
 */
export async function mysqlStandIn(options: { version: string; accepts?: () => string | undefined; passwords?: string[] }): Promise<MysqlStandIn> {
  const queries: string[] = [];
  const tried: string[] = [];
  const sockets = new Set<Socket>();
  let open = 0;
  let ended = false;
  const server = createServer((socket) => {
    sockets.add(socket);
    const salt = Buffer.from(randomBytes(20).map((b) => (b % 94) + 33));
    let buffer = Buffer.alloc(0);
    let authed = false;
    const send = (seq: number, payload: Buffer) => {
      const head = Buffer.alloc(4);
      head.writeUIntLE(payload.length, 0, 3);
      head[3] = seq & 0xff;
      socket.write(Buffer.concat([head, payload]));
    };
    const resultSet = (name: string, value: string) => {
      const column = Buffer.concat([str("def"), str(""), str(""), str(""), str(name), str(""), Buffer.from([0x0c]), u16(33), Buffer.from([255, 0, 0, 0, 0xfd]), u16(0), Buffer.from([0x1f, 0, 0])]);
      [Buffer.from([1]), column, EOF, str(value), EOF].forEach((payload, i) => send(i + 1, payload));
    };
    socket.on("close", () => {
      sockets.delete(socket);
      if (authed) open--;
    });
    socket.on("error", () => {});
    socket.on("data", (data: Buffer) => {
      buffer = Buffer.concat([buffer, data]);
      while (buffer.length >= 4) {
        const length = buffer.readUIntLE(0, 3);
        if (buffer.length < 4 + length) return;
        const payload = buffer.subarray(4, 4 + length);
        buffer = buffer.subarray(4 + length);
        if (!authed) {
          // HandshakeResponse41: capabilities, packet size, charset, 23 bytes of filler, then the user and the scramble.
          let at = 32;
          at = payload.indexOf(0, at) + 1;
          // The scramble's length is one byte either way: a length-encoded integer under 251, as a 20-byte scramble's is.
          const scramble = payload.subarray(at + 1, at + 1 + payload[at]!);
          const name = (options.passwords ?? []).find((p) => scrambled(p, salt).equals(scramble)) ?? (scramble.length ? "?" : "");
          tried.push(name);
          const wanted = options.accepts?.();
          if (wanted !== undefined && !scrambled(wanted, salt).equals(scramble)) {
            send(2, err(1045, "28000", "Access denied for user 'root'@'localhost' (using password: YES)"));
            socket.end();
            return;
          }
          authed = true;
          open++;
          send(2, OK);
          continue;
        }
        const command = payload[0];
        const text = payload.subarray(1).toString("utf8");
        if (command === 0x01) {
          ended = true;
          socket.end();
        } else if (command === 0x03) {
          queries.push(text);
          resultSet(text === "select version()" ? "version()" : "x", text === "select version()" ? options.version : "x");
        } else if (command === 0x0e) {
          send(1, OK);
        } else if (command === 0x16) {
          // The stand-in prepares nothing: the connection goes, as a server's would.
          queries.push(text);
          socket.destroy();
        } else if (command !== 0x19) {
          send(1, err(1047, "08S01", "Unknown command"));
        }
      }
    });
    // HandshakeV10: the version, a connection id, the scramble in two parts, the capabilities and the plugin.
    send(0, Buffer.concat([Buffer.from([10]), Buffer.from(`${options.version}\0`), Buffer.from([1, 0, 0, 0]), salt.subarray(0, 8), Buffer.from([0]), u16(CAPABILITIES & 0xffff), Buffer.from([33]), u16(2), u16(CAPABILITIES >>> 16), Buffer.from([21]), Buffer.alloc(10), salt.subarray(8), Buffer.from([0]), Buffer.from("mysql_native_password\0")]));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    port: (server.address() as AddressInfo).port,
    queries,
    tried,
    open: () => open,
    ended: () => ended,
    close: async () => {
      for (const socket of sockets) socket.destroy();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
