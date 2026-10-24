-- Functions and an operator created in pg_catalog itself, as a superuser or an extension whose
-- control file names that schema may, each of narrower types than a polymorphic built-in and so a
-- closer match of a call over those types: none of them is a call the trust query makes, so it
-- still reports a user's `md5(int4)`, NULL for anything, with its argument types. (The other
-- catalog queries call `unnest` of other arrays and `array_agg`, so those are left alone here.)
create function pg_catalog.unnest(oidvector) returns setof oid language sql immutable as $$ select 0::oid where false $$;
create function pg_catalog.cardinality(int8[]) returns int4 language sql immutable as $$ select 7 $$;
create function pg_catalog.contained_never(oid[], oid[]) returns bool language sql immutable as $$ select false $$;
create operator pg_catalog.<@ (leftarg = oid[], rightarg = oid[], function = pg_catalog.contained_never);
create function md5(int4) returns text language sql immutable as $$ select null::text $$;
create table t (id int primary key, c int4 not null, s text not null);
