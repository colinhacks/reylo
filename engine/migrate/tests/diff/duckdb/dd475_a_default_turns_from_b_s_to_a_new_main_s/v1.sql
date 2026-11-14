create schema b; create sequence b.s; create table b.p2 (id integer primary key, k bigint default nextval('b.s'));
