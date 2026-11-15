create schema s; create macro x(v) as v > 0; create macro s.x(v) as v >= 0; create table t (id integer primary key, parent integer references t (id), v integer, check (s.x(v)));
