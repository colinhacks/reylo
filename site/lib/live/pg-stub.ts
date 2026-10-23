// pg_jsonschema exists for PGlite in no form. The engine types a JSON column from a CHECK that
// calls `jsonb_matches_schema` when the catalog says the function belongs to the extension named
// pg_jsonschema (engine/core/src/schema_type.rs, postgres_validator; CHECKS_QUERY reads the
// extension's name and version through pg_depend), never by running it. So a stub extension of
// that name and version, with the four functions of pg_jsonschema 0.3.4's script under the same
// signatures, volatility and strictness, gives the engine the same catalog rows. Its bodies raise:
// nothing in the playground ever runs a statement, and a stub must never pass for a validator.
export const STUB_VERSION = "0.3.4";
export const STUB_CONTROL = `comment = 'pg_jsonschema (a stub for the Reylo playground: the catalog of 0.3.4, no validator)'
default_version = '${STUB_VERSION}'
relocatable = false
superuser = true
`;
const body = `language plpgsql as $$ begin raise exception 'pg_jsonschema is a stub in the Reylo playground'; end $$`;
export const STUB_SQL = `
create function json_matches_schema("schema" json, "instance" json) returns bool immutable strict parallel safe ${body};
create function jsonb_matches_schema("schema" json, "instance" jsonb) returns bool immutable strict parallel safe ${body};
create function jsonschema_is_valid("schema" json) returns bool immutable strict parallel safe ${body};
create function jsonschema_validation_errors("schema" json, "instance" json) returns text[] immutable strict parallel safe ${body};
`;
const DIR = "/pglite/share/postgresql/extension";

/** A PGlite extension that only writes the stub's control file and script into PGlite's filesystem before Postgres starts. */
export const pgJsonschemaStub = {
  name: "pg_jsonschema (stub)",
  setup: async (_pg: unknown, emscriptenOpts: any) => ({
    emscriptenOpts: {
      ...emscriptenOpts,
      preRun: [
        ...(emscriptenOpts.preRun ?? []),
        (mod: any) => {
          mod.FS.mkdirTree(DIR);
          mod.FS.writeFile(`${DIR}/pg_jsonschema.control`, STUB_CONTROL);
          mod.FS.writeFile(`${DIR}/pg_jsonschema--${STUB_VERSION}.sql`, STUB_SQL);
        },
      ],
    },
  }),
};
