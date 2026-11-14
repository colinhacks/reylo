alter table t alter column n drop default;
drop macro gen;
create macro gen (x) as ((x + 7));
alter table t alter column n set default gen(1);
