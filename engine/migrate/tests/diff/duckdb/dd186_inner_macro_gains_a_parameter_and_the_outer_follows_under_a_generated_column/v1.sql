create macro z_inner(x) as x + 1; create macro a_outer(x) as z_inner(x); create table t (id integer primary key, n integer, m integer generated always as (a_outer(n)) virtual);
