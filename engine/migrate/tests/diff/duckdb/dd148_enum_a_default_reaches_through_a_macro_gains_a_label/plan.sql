alter table t alter column m drop default;
drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column m set default default_mood();
