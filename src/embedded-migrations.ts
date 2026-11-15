// The migrations `migrate(ctx.storage)` runs on a Durable Object (src/migrate.ts). Reylo's Vite and
// esbuild plugins replace this module, in the bundle they build, with the project's locked
// migrations (src/embedded-module.ts); a bundle built without them holds none, and `migrate` says
// so. A bundle that never calls `migrate` leaves the list out, as nothing reads it.
import type { Migration } from "./migrate.ts";

/** The project's migrations in the order they run, why they couldn't be read, or none. */
export const embedded: readonly Migration[] | { readonly error: string } | undefined = undefined;
