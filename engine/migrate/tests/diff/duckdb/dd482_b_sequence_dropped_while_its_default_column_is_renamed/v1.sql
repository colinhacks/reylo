create schema b; create sequence s; create sequence b.s; create table b.p2 (id integer, d bigint default nextval('s'));
