alter table t alter column n drop default;
drop macro a_outer;
drop macro z_inner;
create macro z_inner (k) as (k);
create macro a_outer () as (z_inner(1));
alter table t alter column n set default a_outer();
