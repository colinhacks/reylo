create function f(a bigint, b numeric) returns text language sql immutable return '7';
create table t (id int primary key, n numeric default f(7, 7.5)::numeric) partition by list (id);
create table t_p0 partition of t for values in (1, 2, 3);
