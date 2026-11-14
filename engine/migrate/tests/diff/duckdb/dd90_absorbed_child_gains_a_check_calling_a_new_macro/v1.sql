create table p (id integer primary key, n integer); create table c (id integer, p_id integer references p (id), v integer);
