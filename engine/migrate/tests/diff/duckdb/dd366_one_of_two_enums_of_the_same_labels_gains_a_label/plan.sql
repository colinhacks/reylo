drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column m set data type mood;
