create macro dbl(x) as x * 2; create table t (id integer primary key, n integer, d integer generated always as (dbl(n)), note varchar);
