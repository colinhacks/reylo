create macro x(v) as v > 0; create table t (id integer primary key, v integer, check (x(v)));
