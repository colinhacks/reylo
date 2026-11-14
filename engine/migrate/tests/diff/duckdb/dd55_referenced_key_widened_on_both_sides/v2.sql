create table a (id bigint primary key); create table b (id integer, a_id bigint references a (id));
