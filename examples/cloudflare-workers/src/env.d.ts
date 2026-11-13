// The Worker's bindings, from wrangler.jsonc (the type of `env`), and its main module (the type of
// `exports` in the tests).
declare namespace Cloudflare {
  interface Env {
    DB: D1Database;
    BOARD: DurableObjectNamespace<import("./worker").Board>;
  }
  interface GlobalProps {
    mainModule: typeof import("./worker");
  }
}
