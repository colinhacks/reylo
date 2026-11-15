create schema a; create sequence a.s; create table a.t (id bigint primary key default nextval('s'), n integer);
