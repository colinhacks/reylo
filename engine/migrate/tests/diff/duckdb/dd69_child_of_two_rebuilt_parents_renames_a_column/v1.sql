create table a (id bigint primary key, n integer); create table b (id bigint primary key, n integer); create table c (a_id bigint references a (id), b_id bigint references b (id), note varchar);
