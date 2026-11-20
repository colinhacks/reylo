create table t (a int, b int);
create index on t (a) where a > 0;
create index on t (a, b);
create index on t (b);
