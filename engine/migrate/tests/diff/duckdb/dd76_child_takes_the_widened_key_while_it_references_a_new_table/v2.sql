create table a (id bigint primary key, n integer); create table n (id integer primary key); create table b (id integer, a_id bigint references a (id), n_id integer references n (id));
