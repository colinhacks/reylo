drop type by_day;
drop type mood;
create type mood as enum('a', 'b', 'c');
create type by_day as map(varchar, enum('a', 'b', 'c'));
alter table t alter column m set data type by_day;
