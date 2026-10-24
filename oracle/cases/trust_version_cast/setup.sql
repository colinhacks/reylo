-- A cast of a user's from text to int4, 0 for anything, which no built-in pg_cast row stands in
-- the way of: the trust query reads the server's version without it, so a server of 18 still
-- answers a JSON null cast to a number with NULL, not with the error a server before 18 raised.
create function text_int4_zero(text) returns int4 language sql immutable as $$ select 0 $$;
create cast (text as int4) with function text_int4_zero(text);
create table t (id int primary key);
