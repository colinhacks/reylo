import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Ajv2020 } from "ajv/dist/2020.js";
import { Session } from "../src/analyze.ts";
import { validate } from "../src/json-schema.ts";
import { sqlModule } from "../src/loader.ts";
import { querySchemas, typeSchema } from "../src/schema.ts";
import type { EngineType } from "../src/engine-type.ts";
import { DRIVERS, INDEX, MYSQL, driver, mysqlDatabase, mysqlLive } from "./helpers.ts";

const int4 = (facts: object): EngineType => ({ kind: "integer", bits: 32, label: "int4", ...facts }) as EngineType;
const varchar = (facts: object): EngineType => ({ kind: "text", label: "varchar", ...facts }) as EngineType;
const output = (t: EngineType, inJson = false) => typeSchema(t, "postgres", false, inJson);

test("a text's size in code points and a LIKE pattern are written, and nothing else of a string", () => {
  assert.deepEqual(output(varchar({ size: { max: 20 } })), { type: "string", maxLength: 20 });
  assert.deepEqual(output(varchar({ size: { exact: 5 } })), { type: "string", minLength: 5, maxLength: 5 });
  assert.deepEqual(typeSchema(varchar({ size: { exact: 5 } }), "postgres", true), { type: "string", maxLength: 5 });
  assert.deepEqual(output(varchar({ size: { unit: "byte", encoding: "UTF-8", max: 63 } })), { type: "string" });
  assert.deepEqual(output(varchar({ pattern: { dialect: "postgres", source: "^a" } })), { type: "string" });
  assert.deepEqual(output(varchar({ pattern: { dialect: "ilike", source: "a%" } })), { type: "string" });
});

test("a LIKE pattern reads as the regular expression matching the same whole values", () => {
  const regex = (source: string) => new RegExp((output(varchar({ pattern: { dialect: "like", source } })) as { pattern: string }).pattern, "u");
  assert.equal(regex("a%").source, "^a[\\s\\S]*$");
  assert.ok(regex("%@%").test("a\n@b"));
  assert.ok(!regex("%@%").test("ab"));
  assert.ok(regex("a_c").test("a🙂c"));
  assert.ok(!regex("a_c").test("ac"));
  assert.ok(regex("50\\%").test("50%"));
  assert.ok(!regex("50\\%").test("500"));
  assert.ok(regex("a.c").test("a.c"));
  assert.ok(!regex("a.c").test("abc"));
  assert.ok(regex("(x)[y]").test("(x)[y]"));
});

test("a number's facts are written where its form here is a JSON number and they fit one exactly", () => {
  assert.deepEqual(output(int4({ min: "0", max: "150" })), { type: "integer", format: "int32", minimum: 0, maximum: 150 });
  assert.deepEqual(output(int4({ multiple_of: "5" })), { type: "integer", format: "int32", multipleOf: 5 });
  assert.deepEqual(output(int4({ enum: ["1", "2"] })), { type: "integer", format: "int32", enum: [1, 2] });
  const int8 = { kind: "integer", bits: 64, label: "int8" };
  assert.deepEqual(output({ ...int8, min: "1", max: "9223372036854775806" } as EngineType), { type: "integer", format: "int64", minimum: 1 });
  const float8 = { kind: "float", exponent_bits: 11, mantissa_bits: 52, label: "float8" };
  assert.deepEqual(output({ ...float8, exclusive_min: "0", max: "1.5" } as EngineType), { type: "number", format: "double", exclusiveMinimum: 0, maximum: 1.5 });
  // A decimal `.run()` gives as its text takes no bounds, and inside a document it is a number.
  const numeric = { kind: "decimal", label: "numeric", min: "0", exclusive_max: "1e131072" } as EngineType;
  assert.deepEqual(output(numeric), { type: "string", format: "decimal" });
  assert.equal((output(numeric, true) as { minimum?: number }).minimum, 0);
  assert.equal((output(numeric, true) as { exclusiveMaximum?: number }).exclusiveMaximum, undefined);
});

test("an array's size is written on an array", () => {
  const vector = { kind: "array", label: "vector", element: { type: { kind: "float", exponent_bits: 8, mantissa_bits: 23 } }, size: { exact: 3 } } as EngineType;
  const schema = output(vector) as { minItems?: number; maxItems?: number };
  assert.deepEqual([schema.minItems, schema.maxItems], [3, 3]);
});

test("a nullable position keeps its facts beside null, and comments describe what they are on", () => {
  const schemas = querySchemas(
    {
      comment: "One user",
      params: [
        { name: "age", type: int4({ min: "0" }), nullable: true, comment: "Their age" },
        { name: "doc", type: { kind: "json", label: "jsonb" } as EngineType, schema: { type: "object", description: "As written" }, comment: "Column comment" },
      ],
      columns: [{ name: "name", type: varchar({ size: { max: 20 } }), nullable: true, comment: "Shown on the profile page" }],
      cardinality: "one",
    },
    "postgres",
  ) as { input: { description?: string; properties: Record<string, unknown> }; output: { description?: string; properties: Record<string, unknown> } };
  assert.equal(schemas.input.description, "One user");
  assert.equal(schemas.output.description, "One user");
  assert.deepEqual(schemas.input.properties.age, { description: "Their age", anyOf: [{ type: "integer", format: "int32", minimum: 0 }, { type: "null" }] });
  assert.deepEqual(schemas.input.properties.doc, { description: "As written", type: "object" });
  assert.deepEqual(schemas.output.properties.name, { description: "Shown on the profile page", anyOf: [{ type: "string", maxLength: 20 }, { type: "null" }] });
});

test("a comment stands over a description the type gives, and beneath one the statement's author wrote", () => {
  const timestamp = { kind: "datetime", label: "timestamp" } as EngineType;
  const schemas = querySchemas(
    {
      params: [{ name: "at", type: timestamp, comment: "When to look" }],
      columns: [{ name: "at", type: timestamp, comment: "When they signed up" }],
      cardinality: "one",
    } as never,
    "postgres",
  ) as { input: { properties: Record<string, unknown> }; output: { properties: Record<string, { description: string; allOf: { description: string }[] }> } };
  const at = schemas.output.properties.at!;
  assert.equal(at.description, "When they signed up");
  assert.match(at.allOf[0]!.description, /date and time/);
  assert.equal((schemas.input.properties.at as { description: string }).description, "When to look");
});

test("a time with no zone and MySQL's TIME take no format their values would fail", () => {
  const schema = (t: object, dialect: "postgres" | "mysql") => typeSchema(t as EngineType, dialect) as { type: string; format?: string };
  const time = schema({ kind: "time", tick: "1/1000000", max: "24:00:00", label: "time" }, "postgres");
  assert.equal(time.format, undefined);
  assert.equal(validate(time, "10:00:00.5"), undefined);
  const span = schema({ kind: "duration", max: "PT838H59M59S", tick: "1/10000", label: "time" }, "mysql");
  assert.equal(span.format, undefined);
  for (const value of ["-838:59:59.0000", "838:59:59", "12:00:00.1235"]) assert.equal(validate(span, value), undefined);
});

test("MySQL: a column's definition reaches a query's JSON Schema as a length in code points alone, and every row `.run()` returns holds to it, on mysql2 and mariadb", { skip: mysqlLive, timeout: 60_000 }, async () => {
  // Each column at its longest in characters of the most bytes it stores, and each decimal at its most digits.
  const own = await mysqlDatabase(
    "reylo_schema_facts",
    `create table sized (id int primary key, name varchar(20) not null, code char(5), legacy varchar(10) character set latin1, body text, old_body text character set latin1, price decimal(10,2), part decimal(5,5), at datetime(3), span time(4));
    insert into sized values
      (1, repeat('😀', 20), 'abcde', repeat('é', 10), repeat('😀', 16383), repeat('é', 65535), 99999999.99, 0.99999, '2026-01-01 12:00:00.123', '-838:59:59.0000'),
      (2, '', 'ab   ', 'a', repeat('z', 65535), '', -99999999.99, -0.00001, '2026-01-01 12:00:00', '12:00:00.1235'),
      (3, 'x', null, null, null, null, null, null, null, null)`,
  );
  const root = mkdtempSync(join(tmpdir(), "reylo-schema-facts-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${own.url}\n`);
  mkdirSync(join(root, "node_modules"));
  for (const name of ["mysql2", "mariadb"]) symlinkSync(join(DRIVERS, name), join(root, "node_modules", name));
  const built = async (name: string, sql: string) => {
    const file = join(root, `${name}.sql`);
    writeFileSync(file, sql);
    const { code } = await sqlModule(file, sql);
    const out = join(root, `${name}.mjs`);
    writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
    return (await import(pathToFileURL(out).href)).default;
  };
  const u = new URL(MYSQL!);
  const mysql2 = await driver("mysql2/promise").createConnection(own.url);
  const mariadb = await driver("mariadb").createConnection({ host: u.hostname, port: Number(u.port), user: decodeURIComponent(u.username) || "root", password: decodeURIComponent(u.password), database: own.url.split("/").pop() });
  try {
    const columns = await built("columns", "select name, code, legacy, body, old_body, price, part, at, span from sized order by id");
    const derived = await built("derived", "select d.body from (select id, body from sized) d order by d.id");
    const document = await built("document", "select json_object('name', name, 'code', code, 'body', body, 'price', price) as doc from sized order by id");
    const names = await built("names", "select json_arrayagg(name) as names from sized");
    const nullable = (schema: object) => ({ anyOf: [schema, { type: "null" }] });
    const column = columns.outputSchema.items.properties;
    assert.deepEqual(column.name, { type: "string", maxLength: 20 });
    // Neither is exact: MySQL strips a CHAR's padding as it reads one.
    assert.deepEqual(column.code, nullable({ type: "string", maxLength: 5 }));
    assert.deepEqual(column.legacy, nullable({ type: "string", maxLength: 10 }));
    // A TEXT that stores UTF-8 is sized in bytes, which JSON Schema has no keyword for; any other, and
    // one read through a derived table, in code points.
    assert.deepEqual(column.body, { type: ["string", "null"] });
    assert.deepEqual(column.old_body, nullable({ type: "string", maxLength: 65535 }));
    assert.deepEqual(derived.outputSchema.items.properties.body, nullable({ type: "string", maxLength: 65535 }));
    // A decimal's precision and scale have no keyword, as its text or inside a document.
    assert.deepEqual(column.price, nullable({ type: "string", format: "decimal" }));
    assert.deepEqual(column.part, nullable({ type: "string", format: "decimal" }));
    const leaf = document.outputSchema.items.properties.doc.properties;
    assert.deepEqual(leaf.name, { type: "string", maxLength: 20 });
    assert.deepEqual(leaf.code, nullable({ type: "string", maxLength: 5 }));
    assert.deepEqual(leaf.body, nullable({ type: "string", maxLength: 65535 }));
    assert.deepEqual(leaf.price, nullable({ type: "number", format: "decimal" }));
    assert.deepEqual(names.outputSchema.properties.names, nullable({ type: "array", items: { type: "string", maxLength: 20 } }));

    // The schema a client holds a row to reads it once JSON. A validator counts a length in code
    // points, as JSON Schema says, so twenty emoji are twenty.
    const ajv = new Ajv2020({ strict: false, validateFormats: false, logger: false });
    const holds = async (client: object) => {
      for (const query of [columns, derived, document, names]) {
        const result = JSON.parse(JSON.stringify(await query.run(client)));
        assert.equal(validate(query.outputSchema, result), undefined);
        assert.ok(ajv.validate(query.outputSchema, result), ajv.errorsText());
      }
    };
    for (const client of [mysql2, mariadb]) {
      await holds(client);
      const rows = await columns.run(client);
      assert.deepEqual(rows.map((r: { code: string | null }) => r.code), ["abcde", "ab", null]);
      assert.equal([...rows[0].name].length, 20);
      assert.equal(rows[0].old_body.length, 65535);
      assert.deepEqual(rows.map((r: { price: string | null }) => r.price), ["99999999.99", "-99999999.99", null]);
    }
    // A row past a bound is refused, so the rows above were held to it.
    const [first] = await columns.run(mysql2);
    assert.equal(validate(columns.outputSchema, [{ ...first, label: "x".repeat(21) }])?.path, "/0/name");
    assert.notEqual(validate(columns.outputSchema, [{ ...first, code: "abcdef" }]), undefined);
    // A session that keeps a CHAR's padding gets it to exactly its length, which is within it.
    await mysql2.query("set session sql_mode = concat(@@session.sql_mode, ',PAD_CHAR_TO_FULL_LENGTH')");
    assert.equal((await columns.run(mysql2))[1].code, "ab   ");
    await holds(mysql2);
  } finally {
    await mysql2.end();
    await mariadb.end();
    for (const session of Session.opened()) await session.close();
    await own.drop();
  }
});
