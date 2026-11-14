create schema a; create sequence s start 100; create sequence a.s start 500; create table a.t (id bigint primary key default nextval('s'), n integer);
