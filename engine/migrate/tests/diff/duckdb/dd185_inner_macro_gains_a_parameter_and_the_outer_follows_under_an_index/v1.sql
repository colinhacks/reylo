create macro z_inner(x) as x + 1; create macro a_outer(x) as z_inner(x); create table t (id integer primary key, n integer); create index t_n on t (a_outer(n));
