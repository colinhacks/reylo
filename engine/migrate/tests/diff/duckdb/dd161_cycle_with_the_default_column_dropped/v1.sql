create macro x() as 1; create table p (id integer primary key, k integer default x(), j integer); create table t (id integer primary key, p_id integer, v integer, check (v >= x()));
