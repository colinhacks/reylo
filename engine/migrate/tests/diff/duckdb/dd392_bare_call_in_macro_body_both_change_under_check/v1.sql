create macro z_inner(x) as x + 1; create macro a_outer(x) as z_inner(x) > 0; create table t (a integer, check (a_outer(a)));
