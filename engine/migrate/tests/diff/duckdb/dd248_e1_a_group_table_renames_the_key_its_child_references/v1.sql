create macro x() as 1; create table t (id integer primary key, v integer, check (v >= x())); create table c (id integer primary key, t_id integer references t (id));
