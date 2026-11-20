create table t (id int, c timestamp);
create index x on t ((c is null));
alter index x alter column 1 set statistics 2000;
