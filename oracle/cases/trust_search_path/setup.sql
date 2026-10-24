-- A text `||` of a user's, NULL for anything, which a search path naming pg_catalog after public
-- puts before the built-in, for the statements and for the trust query alike, which reads
-- pg_catalog's operators by their schema: its own `||` is not the user's.
create table t (id int8 primary key, a text not null, b text not null);
create function text_cat_null(text, text) returns text language sql immutable as $$ select null::text $$;
create operator public.|| (leftarg = text, rightarg = text, function = text_cat_null);
set search_path = public, pg_catalog;
