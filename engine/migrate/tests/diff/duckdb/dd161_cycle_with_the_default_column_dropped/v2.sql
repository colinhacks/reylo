create macro x() as 2; create table p (id integer primary key, j integer); create table t (id integer primary key, p_id integer references p (id), v integer, check (v >= x()));
