create function f(i int) returns bigint language sql immutable as 'select i::bigint';
create table p (id int) partition by list (id);
create table h partition of p for values in (1);
create index pi on only p (f(id));
create index new_child on h (f(id));
alter index pi attach partition new_child;
