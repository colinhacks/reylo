create sequence s; create table p (id integer primary key, name varchar); create table c (id integer primary key, p_id integer references p (id), no bigint default nextval('s'));
