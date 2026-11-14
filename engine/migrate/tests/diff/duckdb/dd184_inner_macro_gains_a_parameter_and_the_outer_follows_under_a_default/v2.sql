create macro z_inner(k) as k; create macro a_outer() as z_inner(1); create table t (id integer primary key, n integer default a_outer());
