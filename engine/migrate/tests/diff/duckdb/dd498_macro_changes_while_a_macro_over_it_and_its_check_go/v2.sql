create macro m(x) as x > -1; create table t (id integer, n integer, check (m(n)));
