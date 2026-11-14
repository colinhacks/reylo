create macro x() as 2; create macro y() as 2; create table t (id integer primary key, v integer, check (v >= x() and v >= y())); create table n (id integer primary key, w integer, check (w >= y()));
