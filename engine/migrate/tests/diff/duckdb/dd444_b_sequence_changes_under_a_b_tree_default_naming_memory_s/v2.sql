create schema b; create sequence s; create sequence b.s increment by 3; create table b.pages (id integer primary key, parent_id integer references b.pages (id), j bigint default nextval('memory.s'));
