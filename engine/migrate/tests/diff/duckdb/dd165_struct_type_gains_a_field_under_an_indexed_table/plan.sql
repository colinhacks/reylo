drop type rec;
create type rec as struct(a integer, b integer);
drop index t_n;
alter table t alter column r set data type rec;
create index t_n on t(n);
