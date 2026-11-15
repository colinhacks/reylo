drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t alter column n drop default;
drop macro dflt;
create macro dflt () as ('z');
alter table t alter column n set default dflt();
alter table t alter column m set data type mood;
