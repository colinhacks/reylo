create sequence s start 100; create type mood as enum ('a', 'b', 'c'); create table t (id bigint primary key default nextval('s'), m mood);
