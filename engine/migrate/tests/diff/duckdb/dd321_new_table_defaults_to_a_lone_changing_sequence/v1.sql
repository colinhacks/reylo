create sequence zs; create table t (id bigint primary key default nextval('zs'), n integer);
