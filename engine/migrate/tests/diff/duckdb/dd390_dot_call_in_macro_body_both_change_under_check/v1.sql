create macro z_inner(x) as x + 1; create macro a_outer(x) as x.z_inner() > 0; create table t (a integer, check (a_outer(a)));
