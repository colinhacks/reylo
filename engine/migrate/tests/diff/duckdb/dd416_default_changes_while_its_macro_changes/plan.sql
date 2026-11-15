alter table t alter column a drop default;
drop macro m;
create macro m (x) as ((x + 2));
alter table t alter column a set default m(2);
