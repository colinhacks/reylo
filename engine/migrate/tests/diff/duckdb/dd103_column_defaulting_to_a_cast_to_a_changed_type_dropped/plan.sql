-- loses data: t.k
alter table t alter column j drop default;
alter table t alter column k drop default;
drop type mood;
create type mood as enum('a', 'b', 'c');
alter table t drop column k;
alter table t alter column j set default 'x';
