create table t (id int not null, n text default 'x') partition by list (id);
create table t0 partition of t for values in (1, 2);
create table t1 partition of t for values in (3);
