create macro m(x) as x > 0; create macro outer_ok(x) as m(x) and x < 100; create table t (id integer, n integer, check (m(n)), check (outer_ok(n)));
