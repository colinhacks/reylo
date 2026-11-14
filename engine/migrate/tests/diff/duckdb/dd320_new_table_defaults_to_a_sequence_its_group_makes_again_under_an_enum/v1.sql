create type mood as enum ('a', 'b'); create sequence zs; create table t (id bigint primary key default nextval('zs'), m mood);
