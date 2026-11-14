create macro f(a) as a > 0, (a, b) as a > b; create table t (id integer primary key, n integer check (f(n)), m integer check (f(m, 1)));
