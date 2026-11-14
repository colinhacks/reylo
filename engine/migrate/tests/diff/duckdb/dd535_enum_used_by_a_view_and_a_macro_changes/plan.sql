drop type st;
create type st as enum('a', 'b', 'c');
alter table t alter column s set data type st;
