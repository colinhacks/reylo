create macro x(v) as v > 1; create macro w(v) as x(v) and v < 100; create table t (id integer primary key, v integer, check (w(v)));
