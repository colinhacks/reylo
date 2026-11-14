create macro y(a) as a > 1; create macro z(a) as y(a) and a < 50; create table t (id integer primary key, v integer, check (y(v) and z(v)));
