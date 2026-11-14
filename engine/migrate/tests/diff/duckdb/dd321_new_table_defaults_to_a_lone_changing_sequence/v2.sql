create sequence zs increment by 3; create table t (id bigint primary key default nextval('zs'), n integer); create table u (id bigint primary key default nextval('zs'));
