// What some statements' parameters need checked before they are bound: a typed JSON parameter's
// schema (src/json-schema.ts), and a structured parameter's column list and a `::type` written on
// MySQL or SQLite (src/rows.ts). A module a build step writes imports them beside `defineQuery`
// only when its statement has such a parameter (src/loader.ts), and the run time reads them from
// the query. So a bundler leaves them out of an app none of whose statements has one.
import { cloneJson, validate, withDefaults } from "./json-schema.ts";
import { checkCast, prepareRows, variant } from "./rows.ts";

export const checks = { cloneJson, validate, withDefaults, checkCast, prepareRows, variant };

export type Checks = typeof checks;
