alter table t alter column n drop default;
drop macro m;
create macro m (a) as ((a + 1)), (a, b) as ((a * b));
alter table t alter column n set default m(1, 2);
