create schema b; create sequence s; create table b.t (id integer primary key, k bigint default nextval('s'));
