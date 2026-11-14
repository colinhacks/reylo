create schema b; create sequence s start 100; create table b.p2 (id integer primary key, k bigint default nextval('s'));
