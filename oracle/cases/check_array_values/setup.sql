-- A CHECK that holds an array of strings within a list: `<@` finds NULL in no list, so it keeps out
-- both NULL elements and any string the list lacks, as `in (…)` does for a string column.
create domain labels as text[] check (value <@ array['x', 'y']);
create table posts (
  id int primary key,
  tags text[] not null default '{}' check (tags <@ array['a', 'b']),
  sizes varchar[] check (array['s', 'm']::varchar[] @> sizes),
  both_ways text[] check (both_ways <@ array['a', 'b'] and both_ways <@ array['b', 'c']),
  or_null text[] check (or_null <@ array['a'] or or_null is null),
  sized text[] check (cardinality(sized) < 3 and sized <@ array['a']),
  l labels,
  loose text[],
  containing text[] check (containing @> array['a']),
  other text[] check (cardinality(other) < 3),
  numbers int[] check (numbers <@ array[1, 2])
);
insert into posts (id, tags, sizes, both_ways, or_null, sized, l, loose, containing, other, numbers) values
  (1, '{a,b}', '{s}', '{b}', '{a}', '{a}', '{x}', '{NULL,z}', '{a,z}', '{NULL}', '{1}'),
  (2, '{}', null, null, null, null, null, null, null, null, null);
-- NULL elements and strings off the list are refused; a second dimension is not, which a read
-- throws on at run time, as with any array column.
do $$
begin
  begin
    insert into posts (id, tags) values (3, '{NULL}');
    raise exception 'the check stored a NULL element';
  exception when check_violation then
  end;
  begin
    insert into posts (id, tags) values (3, '{z}');
    raise exception 'the check stored a string off the list';
  exception when check_violation then
  end;
end $$;
create view post_tags as select id, tags from posts;
-- An element under a nondeterministic collation may equal a listed string and differ from it.
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create table folded (id int primary key, tags text[] collate ci check (tags <@ array['a' collate ci]));
insert into folded values (1, '{A}');
