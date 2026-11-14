create macro m2() as 2; create macro m1() as m2() - 1; create table t (id integer primary key, n integer check (n < m1() + m2() + 10));
