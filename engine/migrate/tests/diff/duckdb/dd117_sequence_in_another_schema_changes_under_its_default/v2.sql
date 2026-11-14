create schema b; create sequence b.ids increment by 3; create table b.t (id bigint primary key default nextval('b.ids'), n integer); create index b_t_n on b.t (n);
