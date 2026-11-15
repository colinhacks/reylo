create schema b; create sequence s; create sequence b.s start 100; create table b.pages (id integer primary key, parent_id integer references b.pages (id), k bigint default nextval('s'));
