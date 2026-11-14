create macro y(a) as a > 0; create macro z(a) as y(a) and a < 100; create table t (id integer primary key, v integer, check (y(v)));
