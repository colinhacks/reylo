-- A CHECK that `array_position(col, NULL) IS NULL` keeps every element of an array column
-- non-null: validated, enforced, among the conjuncts of a CHECK on that column alone, or of the
-- column's domain's. A column without one, a CHECK a NULL element passes, and a CHECK that calls a
-- user's `array_position` leave the elements nullable.
create domain labels as text[] check (array_position(value, null) is null);
create table posts (
  id int primary key,
  tags text[] not null default '{}' check (array_position(tags, null) is null),
  scores int[] check (cardinality(scores) < 10 and array_position(scores, null) is null),
  l labels,
  loose text[],
  other text[] check (not (null = any(other)))
);
insert into posts values (1, '{a,b}', '{1}', '{x}', '{NULL,z}', '{q}');
create view post_tags as select id, tags from posts;
-- A user's overload, which the CHECK below resolves to: it answers NULL, so every array passes.
create function public.array_position(text[], text) returns int language sql immutable as 'select null::int';
create table shadowed (id int primary key, tags text[] check (array_position(tags, null) is null));
insert into shadowed values (1, '{NULL}');
-- A NULL under a user's cast may not be NULL: `to_wrap` turns any text, NULL included, into a
-- row, so the CHECK on `wrapped` compares with that row and lets NULL elements through. The server
-- prints a NULL of a domain as the same two casts, which call no function and keep it NULL.
create type wrap as (v text);
create function to_wrap(text) returns wrap language sql immutable as $$ select row('x')::wrap $$;
create cast (text as wrap) with function to_wrap(text);
create domain score as int;
create table casts (
  id int primary key,
  wrapped wrap[] check (array_position(wrapped, null::text::wrap) is null),
  points score[] check (array_position(points, null::score) is null)
);
insert into casts values (1, '{NULL}', '{1}');
-- Postgres refuses to search an array of more than one dimension, so the same CHECK keeps such a
-- value out of the column: it holds one dimension, or none, with no NULL element.
create table grids (id int primary key, cells int[] check (array_position(cells, null) is null));
insert into grids values (1, '{1,2}');
do $$
begin
  insert into grids values (2, '{{1,2},{3,4}}');
  raise exception 'a CHECK on array_position stored an array of two dimensions';
exception when feature_not_supported then
end $$;
