alter table t alter column a drop default;
alter table t alter column b drop default;
alter table t alter column c drop default;
drop macro m;
create macro m (x) as ((x + 2));
alter table t alter column a set default m(1);
alter table t alter column b set default memory.m(2);
alter table t alter column c set default memory.main.m(3);
