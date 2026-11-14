create macro m1() as 1; create macro m2() as m1() + 1; create table t (id integer primary key, n integer check (n < m1() + m2() + 10));
