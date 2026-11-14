create schema a; create sequence a.s increment by 5; create table a.t (id bigint primary key default nextval('s'), n integer);
