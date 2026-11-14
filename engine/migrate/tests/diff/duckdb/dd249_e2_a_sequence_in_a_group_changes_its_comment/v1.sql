create sequence s; create macro x() as 1; create table t (id bigint primary key default nextval('s'), v integer, check (v >= x())); comment on sequence s is 'ids';
