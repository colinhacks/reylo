create schema b; create sequence b.s start 100; create table b.t (id integer primary key, k bigint default nextval('s'));
