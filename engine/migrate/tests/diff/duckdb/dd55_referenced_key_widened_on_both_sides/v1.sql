create table a (id integer primary key); create table b (id integer, a_id integer references a (id));
