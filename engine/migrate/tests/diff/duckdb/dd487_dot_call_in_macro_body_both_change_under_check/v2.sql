create macro z_inner(x) as x + 2; create macro a_outer(x) as x.z_inner() > -1; create table t (id integer, n integer check (a_outer(n)));
