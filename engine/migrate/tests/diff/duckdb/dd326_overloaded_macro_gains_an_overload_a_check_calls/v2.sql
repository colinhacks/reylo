create macro m(a) as a + 1, (a, b) as a + b; create table t (id integer primary key, n integer check (m(n) > 0 and m(n, 1) > 0));
