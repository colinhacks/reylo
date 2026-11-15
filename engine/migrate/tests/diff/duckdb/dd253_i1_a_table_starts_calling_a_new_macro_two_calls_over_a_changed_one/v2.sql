create macro x(v) as v > 1; create macro m2(v) as x(v) and v < 100; create macro m1(v) as m2(v); create table t (id integer primary key, v integer, check (m1(v)));
