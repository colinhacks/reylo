drop type rec;
drop type mood;
create type mood as enum('a', 'b', 'c');
create type rec as struct(m enum('a', 'b', 'c'), n integer);
drop type rec;
create type rec as struct(m enum('a', 'b', 'c'), n integer);
alter table t alter column r set data type rec;
