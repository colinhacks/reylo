create schema a; create sequence s; create sequence a.s; create table a.pages (id bigint primary key default nextval('s'), parent_id bigint references a.pages (id));
