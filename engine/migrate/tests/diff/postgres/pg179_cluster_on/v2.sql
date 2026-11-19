create table t (id int, a int);
create index i on t (a);
alter table t cluster on i;
