create macro z_inner() as 1; create macro a_outer() as z_inner(); create table t (id integer primary key, n integer default a_outer());
