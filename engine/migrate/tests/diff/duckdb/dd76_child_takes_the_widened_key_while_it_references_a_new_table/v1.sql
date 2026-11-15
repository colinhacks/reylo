create table a (id integer primary key, n integer); create table b (id integer, a_id integer references a (id));
