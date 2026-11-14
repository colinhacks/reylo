create macro x() as 2; create table t (id integer primary key, v integer, check (v >= x())); create table n (id integer primary key, t_id integer references t (id), w integer, check (w >= x()));
