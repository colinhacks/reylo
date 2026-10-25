-- A user's `=` of exactly a key's type, true for any operands, which a search path naming
-- pg_catalog after public puts before the built-in: a value of the key's type, a parameter typed
-- from the key and a string literal all resolve to it, and so pin no key. An int4 beside the int8
-- key resolves to the built-in `=` of the two, which matches exactly, and pins it still.
create function eq8_true(int8, int8) returns bool language sql immutable as $$ select true $$;
create operator public.= (leftarg = int8, rightarg = int8, function = eq8_true);
create table t (id bigint primary key, name text not null);
-- An `=` of an enum and a text, true for any operands, which a string literal beside the enum's
-- key resolves to over the built-in `=` of every enum: it matches the enum exactly on the left.
create type mood as enum ('calm', 'busy');
create function mood_text_true(mood, text) returns bool language sql immutable as $$ select true $$;
create operator public.= (leftarg = mood, rightarg = text, function = mood_text_true);
create table e (m mood primary key, n text not null);
insert into e values ('calm', 'a'), ('busy', 'b');
insert into t values (1, 'a'), (2, 'b'), (5, 'c');
-- For the analysis and the checks alike, which run on this session.
set search_path = public, pg_catalog;
