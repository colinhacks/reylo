create schema b; create sequence b.s; create sequence s; create table b.t (id integer primary key, k bigint default nextval('s')); create table u (id integer primary key);
