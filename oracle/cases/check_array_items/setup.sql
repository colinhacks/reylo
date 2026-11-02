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
