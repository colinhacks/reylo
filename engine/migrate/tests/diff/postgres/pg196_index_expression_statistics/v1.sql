create table t (a int, b text);
create index i on t (a, (a + 1), lower(b));
create index j on t ((a * 2));
alter index j alter column 1 set statistics 500;
