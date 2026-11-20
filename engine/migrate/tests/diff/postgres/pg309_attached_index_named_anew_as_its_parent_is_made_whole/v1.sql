create table p (id int) partition by list (id);
create table h partition of p for values in (1);
create table j partition of p for values in (2);
create index pi on only p (id);
create index old_child on h (id);
alter index pi attach partition old_child;
