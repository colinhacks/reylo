create sequence s increment by 2; create table t (id bigint primary key default nextval('s'), m varchar, n integer);
