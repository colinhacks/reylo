drop type mood;
create type mood as enum('sad', 'ok', 'glad');
alter table t alter column m set data type mood;
