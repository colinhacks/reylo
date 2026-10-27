// What the host does to a database's URL before its engine opens it (src/engine.ts). The component
// reaches files through one preopen and opens only TCP sockets (WASI has no Unix sockets), so the
// files a Postgres URL names (`sslrootcert`, `sslcert`, `sslkey`) are resolved as libpq resolves
// them, from this process's working directory, and mapped to the component's paths; and a URL that
// names a Unix socket (Postgres's `host=/dir`, mysql2's `socketPath`) is opened through a bridge, a
// TCP port on the loopback interface that this thread pipes to the socket. The server still sees a
// connection on its socket, so peer authentication and `'user'@'localhost'` hold as before.
import { createServer, connect, type AddressInfo, type Socket } from "node:net";
import { resolve } from "node:path";
import { guestPath } from "./component.ts";

/** A URL as the component opens it, and what to close with the connection. */
export type HostUrl = { url: string; close(): void };

/** The Postgres URL parameters that name a file. */
const FILES = ["sslrootcert", "sslcert", "sslkey"];

/** `url` as the component opens it: files resolved and mapped, a Unix socket bridged. */
export async function hostUrl(url: string): Promise<HostUrl> {
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
  const socket = postgres ? postgresSocket(base, pairs) : mysqlSocket(pairs);
  if (!socket) return { url: join(base, pairs), close: () => {} };
  const bridge = await openBridge(socket.path);
  const rest = pairs.filter((pair) => !socket.params.includes(splitPair(pair)[0]));
  return { url: join(withHost(base, `127.0.0.1:${bridge.port}`), rest), close: bridge.close };
}

/** The socket file a Postgres URL names, as libpq reads it: a `host` that is a directory, in the query or percent-encoded in the authority, and the port's file in it. */
function postgresSocket(base: string, pairs: string[]): { path: string; params: string[] } | undefined {
  const params = Object.fromEntries(pairs.map(splitPair));
  const authority = /^[a-z]+:\/\/(?:[^@/?]*@)?([^/?]*)/i.exec(base)?.[1] ?? "";
  const [, encodedHost = "", authorityPort] = /^(.*?)(?::(\d+))?$/.exec(authority) ?? [];
  const dir = params.host?.startsWith("/") ? params.host : decodeURIComponent(encodedHost).startsWith("/") ? decodeURIComponent(encodedHost) : undefined;
  if (!dir) return undefined;
  const port = params.port || authorityPort || "5432";
  return { path: `${dir.replace(/\/$/, "")}/.s.PGSQL.${port}`, params: ["host", "port"] };
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

/** A TCP port on the loopback interface piped to the Unix socket at `path`, one socket per connection, none of which keeps the process alive. */
async function openBridge(path: string): Promise<{ port: number; close(): void }> {
  const open = new Set<Socket>();
  const server = createServer((client) => {
    const upstream = connect(path);
    for (const socket of [client, upstream]) {
      open.add(socket);
      socket.unref();
      socket.on("close", () => open.delete(socket));
      socket.on("error", () => {
        client.destroy();
        upstream.destroy();
      });
    }
    client.pipe(upstream).pipe(client);
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
