create macro y(a, b) as a > b; create macro x(a) as y(a, 0); create table t (id integer primary key, v integer, check (x(v)));
