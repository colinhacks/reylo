create table p (id bigint primary key, n integer); create table c (id bigint primary key, p_id bigint references p (id), note varchar);
