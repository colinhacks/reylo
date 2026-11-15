alter table t alter column a drop default;
alter table t alter column b drop default;
drop macro m;
create macro m (x) as ((x + 5));
alter table t alter column a set default memory.m(1);
alter table t alter column b set default m(2);
