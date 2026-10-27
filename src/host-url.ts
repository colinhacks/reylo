// What the host does to a database's URL before its engine opens it (src/engine.ts). The component
// reaches files through one preopen and opens only TCP sockets (WASI has no Unix sockets), so the
// files a Postgres URL names (`sslrootcert`, `sslcert`, `sslkey`) are resolved as libpq resolves
// them, from this process's working directory, and mapped to the component's paths; and a URL that
// names a Unix socket (Postgres's `host=/dir`, or `PGHOST=/dir` where the URL names no host, and
// mysql2's `socketPath`) is opened through a bridge, a TCP port on the loopback interface that this
// thread pipes to the socket. The server sees a connection on its socket, which peer authentication
// (Postgres's `peer`, MySQL's `auth_socket`) trusts as this process's user, so the bridge admits the
// engine's own connections alone: each carries the bridge's token, which only the engine is given,
// and any other is closed before the server reads a byte of it.
import { randomBytes, timingSafeEqual } from "node:crypto";
import { createServer, connect, type AddressInfo, type Socket } from "node:net";
import { resolve } from "node:path";
import { guestPath } from "./component.ts";

/** A URL as the component opens it, the bridge's token where it goes through one, and what to close with the connection. */
export type HostUrl = { url: string; bridge?: string; close(): void };

/** The Postgres URL parameters that name a file. */
const FILES = ["sslrootcert", "sslcert", "sslkey"];

/** The connection attribute a MySQL login through the bridge carries its token in (engine/native/src/mysql.rs). */
const MYSQL_ATTRIBUTE = "reylo_bridge";

/** How long a connection to the bridge may take to show its token. */
const TOKEN_TIMEOUT = 10_000;

/** `url` as the component opens it: files resolved and mapped, a Unix socket bridged. */
export async function hostUrl(url: string, env: Record<string, string | undefined> = process.env): Promise<HostUrl> {
  const postgres = /^postgres(ql)?:\/\//i.test(url);
  const mysql = /^mysql:\/\//i.test(url);
  if (!postgres && !mysql) return { url, close: () => {} };
  const [base, query = ""] = splitQuery(url);
  let pairs = query ? query.split("&") : [];
  if (postgres) pairs = pairs.map((pair) => {
    const [key, value] = splitPair(pair);
    if (!FILES.includes(key) || value === "" || value === "system") return pair;
    return `${key}=${encodeURIComponent(guestPath(resolve(value)))}`;
  });
  const socket = postgres ? postgresSocket(base, pairs, env) : mysqlSocket(pairs);
  if (!socket) return { url: join(base, pairs), close: () => {} };
  const token = randomBytes(24).toString("hex");
  const bridge = await openBridge(socket.path, postgres ? "postgres" : "mysql", token);
  const rest = pairs.filter((pair) => !socket.params.includes(splitPair(pair)[0]));
  return { url: join(withHost(base, `127.0.0.1:${bridge.port}`), rest), bridge: token, close: bridge.close };
}

/**
 * The socket file a Postgres URL names, as libpq reads it: a `host` that is a directory, in the
 * query or percent-encoded in the authority, or `PGHOST` where the URL names no host at all, and the
 * port's file in it (the URL's port, else `PGPORT`, else 5432).
 */
function postgresSocket(base: string, pairs: string[], env: Record<string, string | undefined>): { path: string; params: string[] } | undefined {
  const params = Object.fromEntries(pairs.map(splitPair));
  const authority = /^[a-z]+:\/\/(?:[^@/?]*@)?([^/?]*)/i.exec(base)?.[1] ?? "";
  const [, encodedHost = "", authorityPort] = /^(.*?)(?::(\d+))?$/.exec(authority) ?? [];
  const host = params.host ?? (encodedHost ? decodeURIComponent(encodedHost) : params.hostaddr ? undefined : env.PGHOST);
  if (!host?.startsWith("/") || host.includes(",")) return undefined;
  const port = params.port || authorityPort || env.PGPORT || "5432";
  return { path: `${host.replace(/\/$/, "")}/.s.PGSQL.${port}`, params: ["host", "port", "sslmode"] };
}

/** The socket file a mysql2 URL names: `socketPath`, or `socket`. */
function mysqlSocket(pairs: string[]): { path: string; params: string[] } | undefined {
  const params = Object.fromEntries(pairs.map(splitPair));
  const path = params.socketPath || params.socket;
  return path ? { path, params: ["socketPath", "socket"] } : undefined;
}

function splitQuery(url: string): [string, string?] {
  const q = url.indexOf("?");
  return q < 0 ? [url] : [url.slice(0, q), url.slice(q + 1)];
}

function splitPair(pair: string): [string, string] {
  const eq = pair.indexOf("=");
  const decode = (s: string) => {
    try {
      return decodeURIComponent(s);
    } catch {
      return s;
    }
  };
  return eq < 0 ? [decode(pair), ""] : [decode(pair.slice(0, eq)), decode(pair.slice(eq + 1))];
}

function join(base: string, pairs: string[]): string {
  return pairs.length ? `${base}?${pairs.join("&")}` : base;
}

/** `base` with its host and port replaced, its user, password and path kept. */
function withHost(base: string, host: string): string {
  return base.replace(/^([a-z]+:\/\/(?:[^@/?]*@)?)[^/?]*/i, `$1${host}`);
}

/**
 * A TCP port on the loopback interface piped to the Unix socket at `path`, for the connections that
 * carry `token`: on Postgres the engine sends it before anything else; on MySQL, whose server speaks
 * first, the login carries it as a connection attribute, which the bridge reads in the client's
 * handshake before passing the handshake on, having taken TLS out of the server's greeting so that
 * the handshake is in plain text. None of its sockets keeps the process alive.
 */
async function openBridge(path: string, dialect: "postgres" | "mysql", token: string): Promise<{ port: number; close(): void }> {
  const open = new Set<Socket>();
  const track = (socket: Socket) => {
    open.add(socket);
    socket.unref();
    socket.on("close", () => open.delete(socket));
  };
  const server = createServer((client) => {
    track(client);
    client.on("error", () => client.destroy());
    const refused = setTimeout(() => client.destroy(), TOKEN_TIMEOUT);
    refused.unref();
    client.on("close", () => clearTimeout(refused));
    const pipe = (upstream: Socket, toServer: Buffer[]) => {
      clearTimeout(refused);
      for (const chunk of toServer) upstream.write(chunk);
      client.pipe(upstream).pipe(client);
    };
    if (dialect === "postgres") gatePostgres(client, token, () => openUpstream(path, client, track), pipe);
    else gateMysql(client, token, openUpstream(path, client, track), pipe);
  });
  await new Promise<void>((done, fail) => {
    server.once("error", fail);
    server.listen(0, "127.0.0.1", done);
  });
  server.unref();
  return {
    port: (server.address() as AddressInfo).port,
    close: () => {
      server.close();
      for (const socket of open) socket.destroy();
    },
  };
}

/** The connection to the socket for `client`, each closed with the other. */
function openUpstream(path: string, client: Socket, track: (socket: Socket) => void): Socket {
  const upstream = connect(path);
  track(upstream);
  upstream.on("error", () => {
    upstream.destroy();
    client.destroy();
  });
  upstream.on("close", () => client.destroy());
  client.on("close", () => upstream.destroy());
  return upstream;
}

/** A Postgres connection admitted once its first bytes are the token: the socket is opened only then, and what followed the token goes to it first. */
function gatePostgres(client: Socket, token: string, upstream: () => Socket, pipe: (upstream: Socket, toServer: Buffer[]) => void): void {
  const expected = Buffer.from(token);
  let head = Buffer.alloc(0);
  const read = (chunk: Buffer) => {
    head = Buffer.concat([head, chunk]);
    if (head.length < expected.length) return;
    client.off("data", read);
    client.pause();
    if (!timingSafeEqual(head.subarray(0, expected.length), expected)) return void client.destroy();
    pipe(upstream(), [head.subarray(expected.length)]);
    client.resume();
  };
  client.on("data", read);
}

/**
 * A MySQL connection admitted once its login carries the token. The server's greeting reaches the
 * client without TLS among its capabilities, so the login that answers it is in plain text; the
 * login is read whole, and passed on only where its connection attributes hold the token.
 */
function gateMysql(client: Socket, token: string, upstream: Socket, pipe: (upstream: Socket, toServer: Buffer[]) => void): void {
  let fromServer = Buffer.alloc(0);
  const greeting = (chunk: Buffer) => {
    fromServer = Buffer.concat([fromServer, chunk]);
    const packet = whole(fromServer);
    if (!packet) return;
    upstream.off("data", greeting);
    upstream.pause();
    withoutTls(packet.payload);
    client.write(fromServer);
    let fromClient = Buffer.alloc(0);
    const login = (chunk: Buffer) => {
      fromClient = Buffer.concat([fromClient, chunk]);
      const answer = whole(fromClient);
      if (!answer) return;
      client.off("data", login);
      client.pause();
      if (attribute(answer.payload, MYSQL_ATTRIBUTE) !== token) return void client.destroy();
      upstream.resume();
      pipe(upstream, [fromClient]);
      client.resume();
    };
    client.on("data", login);
  };
  upstream.on("data", greeting);
}

/** The first whole packet of a MySQL stream, its payload in place, or undefined while it is still arriving. */
function whole(stream: Buffer): { payload: Buffer } | undefined {
  if (stream.length < 4) return undefined;
  const length = stream.readUIntLE(0, 3);
  return stream.length < 4 + length ? undefined : { payload: stream.subarray(4, 4 + length) };
}

/** The greeting (HandshakeV10) with CLIENT_SSL cleared from its capabilities, in place. */
function withoutTls(greeting: Buffer): void {
  const at = greeting.indexOf(0, 1) + 1 + 4 + 8 + 1;
  if (greeting[0] === 10 && at + 2 <= greeting.length) greeting.writeUInt16LE(greeting.readUInt16LE(at) & ~0x0800, at);
}

/** A connection attribute of a login (HandshakeResponse41), or undefined where it carries none. */
export function attribute(login: Buffer, name: string): string | undefined {
  const CONNECT_WITH_DB = 0x8, SSL = 0x800, SECURE_CONNECTION = 0x8000, PLUGIN_AUTH = 0x80000, CONNECT_ATTRS = 0x100000, LENENC_AUTH = 0x200000;
  if (login.length < 32) return undefined;
  const capabilities = login.readUInt32LE(0);
  if (capabilities & SSL || !(capabilities & CONNECT_ATTRS)) return undefined;
  // Capabilities, the largest packet, the character set and 23 bytes of filler.
  let at = 32;
  const int = (): number => {
    const first = login[at++];
    if (first === undefined) throw new RangeError("short");
    if (first < 0xfb) return first;
    const size = first === 0xfc ? 2 : first === 0xfd ? 3 : 8;
    const value = login.readUIntLE(at, Math.min(size, 6));
    at += size;
    return value;
  };
  const bytes = (n: number): Buffer => {
    if (at + n > login.length) throw new RangeError("short");
    const value = login.subarray(at, at + n);
    at += n;
    return value;
  };
  const cstring = (): void => {
    const end = login.indexOf(0, at);
    if (end < 0) throw new RangeError("unterminated");
    at = end + 1;
  };
  try {
    cstring(); // the user
    if (capabilities & LENENC_AUTH) bytes(int());
    else if (capabilities & SECURE_CONNECTION) bytes(bytes(1)[0]!);
    else cstring();
    if (capabilities & CONNECT_WITH_DB) cstring();
    if (capabilities & PLUGIN_AUTH) cstring();
    const length = int();
    const end = at + length;
    while (at < end) {
      const key = bytes(int()).toString("utf8");
      const value = bytes(int()).toString("utf8");
      if (key === name) return value;
    }
  } catch {}
  return undefined;
}
