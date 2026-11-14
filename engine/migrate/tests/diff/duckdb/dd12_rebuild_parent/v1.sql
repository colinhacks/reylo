create table a (id bigint primary key, name varchar); create table b (id bigint primary key, a_id bigint references a (id)); create table c (id bigint, b_id bigint references b (id));
