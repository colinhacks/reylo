create macro z_inner(x, k) as x + k; create macro a_outer(x) as z_inner(x, 1); create table t (id integer primary key, n integer); create index t_n on t (a_outer(n));
