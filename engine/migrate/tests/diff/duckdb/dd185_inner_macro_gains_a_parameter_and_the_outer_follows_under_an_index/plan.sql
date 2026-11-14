drop index t_n;
drop macro a_outer;
drop macro z_inner;
create macro z_inner (x, k) as ((x + k));
create macro a_outer (x) as (z_inner(x, 1));
create index t_n on t((a_outer(n)));
