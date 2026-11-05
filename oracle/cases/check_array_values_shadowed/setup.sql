-- A user's `<@` of `text[]`, an exact match where the built-in takes any array, is the one the CHECK
-- calls: it answers true for anything, so the CHECK keeps nothing out.
create function always(text[], text[]) returns boolean language sql immutable as 'select true';
create operator <@ (leftarg = text[], rightarg = text[], function = always);
create table shadowed (id int primary key, tags text[] check (tags <@ array['a']));
insert into shadowed values (1, '{z,NULL}');
