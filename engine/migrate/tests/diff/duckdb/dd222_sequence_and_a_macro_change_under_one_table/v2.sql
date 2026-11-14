create sequence s start with 10; create macro x() as 2; create table t (id bigint primary key default nextval('s'), v integer, check (v >= x()));
