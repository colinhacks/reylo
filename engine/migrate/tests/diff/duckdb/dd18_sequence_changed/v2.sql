create sequence s start 1 increment 2; create table t (id bigint primary key default nextval('s'), a varchar);
