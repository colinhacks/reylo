create function f(a int) returns int language sql immutable return a;
create table t (id int not null, g int generated always as (f(id)) stored) partition by list (id);
create table t0 partition of t for values in (1, 2, 3);
