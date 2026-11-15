drop type pair;
drop type mood;
create type mood as enum('a', 'b', 'c');
create type pair as struct(m enum('a', 'b', 'c'), n integer);
alter table t alter column p set data type pair;
