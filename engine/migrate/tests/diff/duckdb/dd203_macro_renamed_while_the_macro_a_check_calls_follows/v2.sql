create macro z_new(x) as x > 0; create macro a_outer(x) as z_new(x); create table t (id integer primary key, n integer check (a_outer(n)));
