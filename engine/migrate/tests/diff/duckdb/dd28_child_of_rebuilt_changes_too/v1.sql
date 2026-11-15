create table a (id bigint primary key, n integer); create table b (id bigint primary key, a_id bigint references a (id), note varchar);
