create macro m(a) as a + 1, (a, b) as a * b; create table t (id integer primary key, n integer default m(1, 2));
