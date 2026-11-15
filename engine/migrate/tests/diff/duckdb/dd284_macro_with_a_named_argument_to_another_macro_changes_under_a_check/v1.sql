create macro inc(x, k := 1) as x + k; create macro pos(x) as inc(x, k := 2) > 0; create table t (id integer primary key, n integer check (pos(n)));
