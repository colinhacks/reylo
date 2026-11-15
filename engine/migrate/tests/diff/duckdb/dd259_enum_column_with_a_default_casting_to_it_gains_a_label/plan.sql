alter table t alter column m drop default;
drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column m set default cast('a' as mood);
alter table t alter column m set data type mood;
