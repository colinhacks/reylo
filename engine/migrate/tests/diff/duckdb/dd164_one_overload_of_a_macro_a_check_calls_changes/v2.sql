create macro ok(a) as a > 0, (a, b) as a >= b; create table t (id integer primary key, v integer, check (ok(v)));
