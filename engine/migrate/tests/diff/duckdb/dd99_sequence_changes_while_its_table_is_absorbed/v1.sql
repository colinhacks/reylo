create sequence s; create table p (id integer primary key, n integer); create table c (id bigint default nextval('s'), p_id integer references p (id));
