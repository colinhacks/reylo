create function f(a int) returns text language sql immutable return '7';
create table t (id int not null, g int generated always as (f(id)::int) stored) partition by list (id);
create table t0 partition of t for values in (1, 2, 3);
