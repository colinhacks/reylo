create macro m(a) as a + 1; create table t (id integer primary key, n integer check (m(n) > 0));
