-- A JSON column that a CHECK holds to a JSON Schema through JSON_SCHEMA_VALID is typed by the
-- values the schema admits, as MySQL's validator reads it: draft 4 whatever `$schema` says, so
-- `const` and `prefixItems` are no keywords, and a `$ref` whose pointer has a `$` in it resolves
-- nowhere and admits anything. Only a column the prepare traces to a base table's column is read:
-- through a derived table or a WITH query MySQL names the base table and the derived column's own
-- name. The rows below are values each CHECK lets in that a narrower type would get wrong.
create table docs (id int primary key, doc json not null, alt json, check (json_schema_valid('{"type":"object","properties":{"name":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}},"kind":{"enum":["a","b"]}},"required":["name","kind"]}', doc)));
insert into docs values (1, '{"name": "x", "kind": "a"}', '[1]'), (2, '{"name": "y", "kind": "b", "tags": [], "extra": [1, null]}', 'null');
create table other (id int primary key, doc json);
insert into other values (1, '[1, 2]');
create table events (id int primary key, payload json, check (payload is null or json_schema_valid('{"oneOf":[{"type":"object","properties":{"type":{"enum":["click"]},"x":{"type":"number"}},"required":["type","x"],"additionalProperties":false},{"type":"object","properties":{"type":{"enum":["key"]},"key":{"type":"string"}},"required":["type","key"],"additionalProperties":false}]}', payload)));
insert into events values (1, '{"type": "click", "x": 1.5}'), (2, '{"type": "key", "key": "a"}'), (3, null);
create table consts (id int primary key, c json, check (json_schema_valid('{"const": "a"}', c)));
insert into consts values (1, '"b"'), (2, '[1]');
create table refs (id int primary key, a json, b json, check (json_schema_valid('{"$defs":{"s":{"type":"string"}},"$ref":"#/$defs/s"}', a)), check (json_schema_valid('{"definitions":{"s":{"type":"string"}},"$ref":"#/definitions/s"}', b)));
insert into refs values (1, '1', '"s"');
create table pairs (id int primary key, p json, check (json_schema_valid('{"type":"array","items":[{"type":"string"},{"type":"integer"}],"additionalItems":false}', p)));
insert into pairs values (1, '["a"]'), (2, '["a", 1]'), (3, '[]');
create table words (id int primary key, w json, check (json_schema_valid('{"enum":["it\'s","a\\\\b","café","😀"]}', w)));
insert into words values (1, '"it\'s"'), (2, '"café"'), (3, '"😀"'), (4, '"a\\\\b"');
create view doc_view as select id, doc from docs;
-- A schema that names a key twice in one object is read by its first: `dup` holds strings.
create table repeated (id int primary key, dup json not null, check (json_schema_valid('{"type":"string","type":"integer"}', dup)));
insert into repeated values (1, '"str"');
