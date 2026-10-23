-- A read of a JSON column at literal keys (->, ->>, #>, #>>) holds what the JSON Schema a CHECK
-- holds the column to says is there: never NULL where every step lands on an object that must
-- hold its key, and for a text read only where no JSON null can be there. A key left out, a JSON
-- null, a nested object that may be null, a key no property names, an array a whole-number key
-- indexes and the arms of a tagged union are in the rows below, so a wrong claim meets them.
create extension if not exists pg_jsonschema;
create table docs (
  id int primary key,
  doc jsonb not null check (jsonb_matches_schema('{
    "type": "object",
    "properties": {
      "name": {"type": "string"},
      "kind": {"enum": ["a", "b"]},
      "size": {"type": ["integer", "null"]},
      "tags": {"type": "array", "items": {"type": "string"}},
      "owner": {"type": "object", "properties": {"id": {"type": "integer"}, "email": {"type": "string"}}, "required": ["id"]},
      "meta": {"type": ["object", "null"], "properties": {"note": {"type": "string"}}, "required": ["note"]},
      "0": {"type": "boolean"}
    },
    "required": ["name", "kind", "size", "owner"]
  }', doc)),
  loose jsonb,
  maybe jsonb check (jsonb_matches_schema('{"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]}', maybe))
);
insert into docs values
  (1, '{"name": "x", "kind": "a", "size": null, "owner": {"id": 1}}', null, null),
  (2, '{"name": "", "kind": "b", "size": 3, "tags": ["t", "u"], "owner": {"id": 2, "email": "e"}, "meta": null, "0": true}', '{"x": "s"}', '{"x": 1}'),
  (3, '{"name": "y", "kind": "a", "size": 0, "tags": [], "owner": {"id": 3, "extra": [1]}, "meta": {"note": "n"}, "extra": {"z": 1}}', '[1]', null);
create table events (
  id int primary key,
  payload jsonb not null check (jsonb_matches_schema('{
    "oneOf": [
      {"type": "object", "properties": {"type": {"const": "click"}, "x": {"type": "number"}}, "required": ["type", "x"], "additionalProperties": false},
      {"type": "object", "properties": {"type": {"const": "key"}, "key": {"type": "string"}}, "required": ["type", "key"], "additionalProperties": false}
    ]
  }', payload))
);
insert into events values (1, '{"type": "click", "x": 1.5}'), (2, '{"type": "key", "key": "a"}');
create table notes (id int primary key, body json not null check (json_matches_schema('{"type": "object", "properties": {"title": {"type": "string"}}, "required": ["title"]}', body)));
insert into notes values (1, '{"title": "t"}'), (2, '{"title": "", "title2": 1}');
create table lists (id int primary key, l jsonb not null check (jsonb_matches_schema('{"type": "array", "items": {"type": "object", "properties": {"a": {"type": "integer"}}, "required": ["a"]}}', l)));
insert into lists values (1, '[]'), (2, '[{"a": 1}]');
