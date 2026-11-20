create function f(a bigint) returns numeric language sql immutable as $$ select 7.5 $$;
create table t (id int primary key, n numeric default f(7)) partition by list (id);
create table t_p0 partition of t for values in (1, 2, 3);
