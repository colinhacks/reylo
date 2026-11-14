create macro z_inner(x, k) as x > k; create macro a_outer(x) as z_inner(x, 0); create table t (id integer primary key, n integer, check (a_outer(n)));
