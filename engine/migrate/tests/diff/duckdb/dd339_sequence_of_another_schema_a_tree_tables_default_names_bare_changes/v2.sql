create schema a; create sequence a.s increment by 5; create table a.pages (id bigint primary key default nextval('s'), parent_id bigint references a.pages (id));
