create schema a; create sequence s increment by 5; create sequence a.s; create table a.pages (id bigint primary key default nextval('s'), parent_id bigint references a.pages (id));
