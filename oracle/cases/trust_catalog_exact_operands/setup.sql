-- The trust query joins an operator to its function with `p.oid OPERATOR(pg_catalog.=) o.oprcode`,
-- an `oid` beside a `regproc`, which no built-in `=` takes exactly. A superuser's `=(oid, regproc)`
-- in pg_catalog is then the exact match, and here pairs every operator with `int4pl`, a stable
-- function that returns no set. A user's `+(int4, text)` is volatile: a key compared with it
-- matches a row per value it draws.
create function pg_catalog.oid_regproc_eq(oid, regproc) returns bool language sql immutable as $$ select $1 OPERATOR(pg_catalog.=) 'pg_catalog.int4pl'::pg_catalog.regproc::pg_catalog.oid $$;
create operator pg_catalog.= (leftarg = oid, rightarg = regproc, function = pg_catalog.oid_regproc_eq);
create function rnd_plus(int4, text) returns int4 language sql volatile as $$ select (floor(random() * 3) + 1)::int4 $$;
create operator + (leftarg = int4, rightarg = text, function = rnd_plus);
create table t (id int primary key, name text not null);
insert into t select g, 'n' || g from generate_series(1, 3) g;
