create table t (a int, b text);
create index i on t (a, (a + 1), lower(b));
alter index i alter column 2 set statistics 1000;
alter index i alter column 3 set statistics 100;
create index j on t ((a * 3));
alter index j alter column 1 set statistics 500;
