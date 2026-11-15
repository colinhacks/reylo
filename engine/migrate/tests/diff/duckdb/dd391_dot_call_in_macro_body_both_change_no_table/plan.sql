drop macro a_outer;
drop macro z_inner;
create macro z_inner (x) as ((x + 2));
create macro a_outer (x) as ((x.z_inner() > -1));
