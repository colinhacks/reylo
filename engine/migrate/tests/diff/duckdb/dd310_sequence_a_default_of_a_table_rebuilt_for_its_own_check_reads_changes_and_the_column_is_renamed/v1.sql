create sequence s; create table t (id integer primary key, k bigint default nextval('s'), n integer);
