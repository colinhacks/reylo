create sequence s increment by 2; create table t (id bigint primary key default nextval('s'), v integer, n integer); create index t_i on t (v);
