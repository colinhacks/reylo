create sequence s1; create table t (id bigint primary key default nextval('s1'), a varchar);
