create schema a; create sequence a.s; create sequence s; create table pages (id integer primary key default nextval('s'), parent_id integer references pages (id));
