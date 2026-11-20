create table t (id int not null) partition by range (id);
create table t_p0 partition of t for values from (1) to (4);
create index i0 on t (id);
create index i1 on t (id);
