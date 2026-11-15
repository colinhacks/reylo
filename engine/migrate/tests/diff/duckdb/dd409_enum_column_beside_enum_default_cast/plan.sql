alter table t alter column a drop default;
drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column a set default cast(cast('a' as mood) as VARCHAR);
alter table t alter column c set data type mood;
