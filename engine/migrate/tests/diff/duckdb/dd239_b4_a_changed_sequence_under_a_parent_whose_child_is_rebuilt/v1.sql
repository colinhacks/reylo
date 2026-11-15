create sequence s; create table p (id bigint primary key default nextval('s'), n integer); create table c (id integer primary key, p_id bigint references p (id), v integer);
