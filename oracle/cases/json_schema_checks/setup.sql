-- A json or jsonb column that a CHECK holds to a JSON Schema, through pg_jsonschema's
-- jsonb_matches_schema and json_matches_schema, is typed by the values the schema admits: on a
-- bare read (through a view too), inside a JSON constructor, and for a parameter written whole to
-- it. pg_jsonschema's versions read a schema two ways (draft 7, or 2020-12 by default), and the
-- type is what either admits. The rows below are values each CHECK lets in that a narrower type
-- would get wrong: a key left out, a key no property names, a null where the schema allows one.
create extension if not exists pg_jsonschema;
create table docs (
  id int primary key,
  doc jsonb not null check (jsonb_matches_schema('{
    "type": "object",
    "properties": {
      "name": {"type": "string"},
      "tags": {"type": "array", "items": {"type": "string"}},
      "kind": {"enum": ["a", "b"]},
      "size": {"type": ["integer", "null"]}
    },
    "required": ["name", "kind"]
  }', doc))
);
insert into docs values
  (1, '{"name": "x", "kind": "a"}'),
  (2, '{"name": "y", "kind": "b", "tags": [], "size": null, "extra": [1, {"z": true}]}'),
  (3, '{"name": "", "kind": "a", "tags": ["t"], "size": 3}');
-- A tagged union, NULL let through by the OR, and a closed object.
create table events (
  id int primary key,
  payload jsonb check (payload is null or jsonb_matches_schema('{
    "oneOf": [
      {"type": "object", "properties": {"type": {"const": "click"}, "x": {"type": "number"}}, "required": ["type", "x"], "additionalProperties": false},
      {"type": "object", "properties": {"type": {"const": "key"}, "key": {"type": "string"}}, "required": ["type", "key"], "additionalProperties": false}
    ]
  }', payload))
);
insert into events values (1, '{"type": "click", "x": 1.5}'), (2, '{"type": "key", "key": "a"}'), (3, null);
-- A domain over jsonb, a map of booleans.
create domain flags as jsonb check (jsonb_matches_schema('{"type": "object", "additionalProperties": {"type": "boolean"}}', value));
create table prefs (id int primary key, f flags);
insert into prefs values (1, '{}'), (2, '{"dark": true, "beta": false}');
-- A json column, references into $defs, and recursion cut at its second level.
create table notes (
  id int primary key,
  body json check (json_matches_schema('{
    "$defs": {"node": {"type": "object", "properties": {"label": {"type": "string"}, "children": {"type": "array", "items": {"$ref": "#/$defs/node"}}}, "required": ["label"]}},
    "$ref": "#/$defs/node"
  }', body))
);
insert into notes values (1, '{"label": "root", "children": [{"label": "a", "children": [{"label": "b", "children": []}]}]}');
-- A tuple under 2020-12, which pg_jsonschema before 0.3.4 reads as draft 7: there `prefixItems`
-- is no keyword and `items: false` leaves only the empty array.
create table pairs (id int primary key, p jsonb check (jsonb_matches_schema('{"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}], "items": false}', p)));
insert into pairs values (1, '[]');
-- A schema that admits null, on a NOT NULL column, and a CHECK added NOT VALID, which says
-- nothing of the rows before it.
create table maybes (id int primary key, m jsonb not null check (jsonb_matches_schema('{"type": ["string", "null"]}', m)), old jsonb);
insert into maybes values (1, 'null', '"before"'), (2, '"s"', null);
alter table maybes add constraint old_schema check (jsonb_matches_schema('{"type": "number"}', old)) not valid;
-- A union of a literal, an object and null, which the type lists in that order.
create table sizes (id int primary key, s jsonb not null check (jsonb_matches_schema('{"anyOf": [{"type": "object", "properties": {"w": {"type": "integer"}, "h": {"type": "integer"}}, "required": ["w", "h"], "additionalProperties": false}, {"enum": ["auto", null]}]}', s)));
insert into sizes values (1, '"auto"'), (2, '{"w": 2, "h": 3}'), (3, 'null');
create view doc_view as select id, doc from docs;
