create macro x() as 1; create macro y() as 1; create table t (id integer primary key, v integer, check (v >= x() and v >= y()));
