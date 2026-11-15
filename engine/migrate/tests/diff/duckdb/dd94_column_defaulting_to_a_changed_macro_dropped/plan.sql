-- loses data: t.k
alter table t alter column k drop default;
drop macro m;
create macro m () as (2);
alter table t drop column k;
