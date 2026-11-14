create sequence s; create type mood as enum ('a', 'b'); create table t (id bigint primary key default nextval('s'), m mood);
