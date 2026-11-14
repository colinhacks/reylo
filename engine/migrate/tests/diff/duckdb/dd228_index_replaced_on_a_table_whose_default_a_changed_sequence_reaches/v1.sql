create sequence s; create table t (id bigint primary key default nextval('s'), v integer, n integer); create index t_i on t (n);
