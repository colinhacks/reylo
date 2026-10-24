-- Casts of a user's, NULL for anything, between built-in types no built-in pg_cast row converts
-- (the server converts them through I/O, or element by element): CREATE CAST may take each, and
-- a conversion of the trust query's that went through one would run it. A user's `md5(int4)`,
-- NULL for anything, is the exact match of `md5(c)`, which the trust query must report with its
-- argument types.
create function text_int8_null(text) returns int8 language sql immutable as $$ select null::int8 $$;
create cast (text as int8) with function text_int8_null(text);
create function oidvector_text_null(oidvector) returns text language sql immutable as $$ select null::text $$;
create cast (oidvector as text) with function oidvector_text_null(oidvector);
create function oidvector_oids_null(oidvector) returns oid[] language sql immutable as $$ select null::oid[] $$;
create cast (oidvector as oid[]) with function oidvector_oids_null(oidvector);
create function oids_int8s_null(oid[]) returns int8[] language sql immutable as $$ select null::int8[] $$;
create cast (oid[] as int8[]) with function oids_int8s_null(oid[]);
create function texts_int8s_null(text[]) returns int8[] language sql immutable as $$ select null::int8[] $$;
create cast (text[] as int8[]) with function texts_int8s_null(text[]);
create function md5(int4) returns text language sql immutable as $$ select null::text $$;
create table t (id int primary key, c int4 not null, s text not null);
