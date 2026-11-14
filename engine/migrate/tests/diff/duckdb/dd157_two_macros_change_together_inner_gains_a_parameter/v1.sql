create macro y(a) as a > 0; create macro x(a) as y(a); create table t (id integer primary key, v integer, check (x(v)));
