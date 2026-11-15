create schema b; create sequence s increment by 2; create table b.p2 (id integer, d bigint default nextval('s'));
