create schema a; create sequence a.s increment by 5; create table t (id bigint primary key default nextval('a.s'), n integer);
