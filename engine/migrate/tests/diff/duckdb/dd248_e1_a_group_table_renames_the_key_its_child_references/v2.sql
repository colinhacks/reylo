create macro x() as 2; create table t (tid integer primary key, v integer, check (v >= x())); create table c (id integer primary key, t_id integer references t (tid));
