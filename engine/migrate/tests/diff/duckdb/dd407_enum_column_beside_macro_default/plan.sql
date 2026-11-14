drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column a drop default;
drop macro m;
create macro m (x) as ((x + 2));
alter table t alter column a set default m(1);
alter table t alter column c set data type mood;
