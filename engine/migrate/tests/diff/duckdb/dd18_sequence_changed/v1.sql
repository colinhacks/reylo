create sequence s start 1; create table t (id bigint primary key default nextval('s'), a varchar);
