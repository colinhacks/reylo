drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column mp set data type map(varchar, enum('a', 'b', 'c'));
alter table t alter column l set data type enum('a', 'b', 'c')[];
