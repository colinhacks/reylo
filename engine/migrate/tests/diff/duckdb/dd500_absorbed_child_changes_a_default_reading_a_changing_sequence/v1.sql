create sequence s; create table p (id integer primary key, a integer); create table c (id integer primary key, pid integer references p (id), d bigint default nextval('s'));
