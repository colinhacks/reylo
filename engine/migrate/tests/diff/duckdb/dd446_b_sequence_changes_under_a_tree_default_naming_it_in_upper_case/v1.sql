create schema b; create sequence b.s; create table pages (id integer primary key, parent_id integer references pages (id), k bigint default nextval('B.S'), j bigint default nextval($$b.s$$));
