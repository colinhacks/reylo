create type mood as enum ('a', 'b'); create sequence s; create table t (id bigint primary key default nextval('s'), m mood, n integer);
