-- Operators and functions of a user's, over built-in types, under the names the inference rules
-- read: a call the server may resolve to one of them proves nothing, whether it matches it
-- exactly, reaches it through an implicit cast or takes it for a polymorphic argument, and one
-- that matches a built-in exactly is the built-in still. Each answers NULL for any operands.
create table t (id bigint primary key, big bigint not null, age int not null, name text not null, note text, flag bool not null);
-- An `=` of exactly the built-in's types, which a search path naming pg_catalog after public puts
-- first.
create function eq8(int8, int8) returns bool language sql immutable as $$ select null::bool $$;
create operator public.= (leftarg = int8, rightarg = int8, function = eq8);
-- A `+` of a text and a bigint, which an int reaches through its implicit cast to bigint.
create function plus_null(text, int8) returns int8 language sql immutable as $$ select null::int8 $$;
create operator + (leftarg = text, rightarg = int8, function = plus_null);
-- A `||` of any two values of one type.
create function concat_null(anyelement, anyelement) returns text language sql immutable as $$ select null::text $$;
create operator || (leftarg = anyelement, rightarg = anyelement, function = concat_null);
-- A `lower` of an int, which no built-in takes.
create function lower(int4) returns text language sql immutable as $$ select null::text $$;
-- An implicit cast between two built-in types by a function, which a built-in operator over the
-- one may run to reach the other, and a cast to the other runs.
create function bool_to_int8(bool) returns int8 language sql immutable as $$ select null::int8 $$;
create cast (bool as int8) with function bool_to_int8(bool) as implicit;
create table posts (id bigint primary key, author_id bigint, title text not null);
-- A cast function of a user's to text, as PostGIS has one from geometry: a CHECK's strings still
-- compare with the built-in `=` of two texts, and a cast of any built-in type to text is a
-- built-in one.
create type pair as (a int, b int);
create function pair_text(pair) returns text language sql immutable as $$ select null::text $$;
create cast (pair as text) with function pair_text(pair);
create table accounts (id int primary key, status text not null check (status in ('active', 'banned')));
insert into accounts values (1, 'active'), (2, 'banned');
-- The search path that puts public's `=` of two int8s before pg_catalog's, for the analysis and the
-- checks alike, which run on this session.
set search_path = public, pg_catalog;
