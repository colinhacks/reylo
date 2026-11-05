-- A CHECK of a JSON column's kind types it as that kind, and refuses JSON's null, whose kind is null.
create domain doc_object as jsonb check (jsonb_typeof(value) = 'object');
create table docs (
  id int primary key,
  o jsonb not null default '{}' check (jsonb_typeof(o) = 'object'),
  a jsonb check (jsonb_typeof(a) = 'array'),
  reversed jsonb check ('object' = jsonb_typeof(reversed)),
  plain json check (json_typeof(plain) = 'array'),
  and_more jsonb check (jsonb_typeof(and_more) = 'object' and and_more ? 'x'),
  or_null jsonb check (jsonb_typeof(or_null) = 'array' or or_null is null),
  d doc_object,
  loose jsonb,
  or_other jsonb check (jsonb_typeof(or_other) = 'object' or jsonb_typeof(or_other) = 'array'),
  not_kind jsonb check (jsonb_typeof(not_kind) <> 'object'),
  number jsonb check (jsonb_typeof(number) = 'number')
);
insert into docs values
  (1, '{"x": 1}', '[1, "a"]', '{}', '[null]', '{"x": null}', '[]', '{"y": []}', 'null', '[]', '1', '1'),
  (2, '{}', null, null, null, null, null, null, null, null, null, null);
do $$
begin
  begin
    insert into docs (id, o) values (3, 'null');
    raise exception 'the check stored JSON null';
  exception when check_violation then
  end;
  begin
    insert into docs (id, o) values (3, '[]');
    raise exception 'the check stored an array';
  exception when check_violation then
  end;
end $$;
-- A user's `jsonb_typeof`, which the CHECK below resolves to.
create schema own;
create function own.jsonb_typeof(jsonb) returns text language sql immutable as $$ select 'object' $$;
create table shadowed (id int primary key, doc jsonb check (own.jsonb_typeof(doc) = 'object'));
insert into shadowed values (1, '[1]');
