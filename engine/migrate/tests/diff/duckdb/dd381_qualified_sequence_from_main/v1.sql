create schema a; create sequence a.s; create table t (id bigint primary key default nextval('a.s'), n integer);
