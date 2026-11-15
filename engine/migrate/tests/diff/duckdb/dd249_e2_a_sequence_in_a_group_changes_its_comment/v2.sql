create sequence s increment by 2; create macro x() as 2; create table t (id bigint primary key default nextval('s'), v integer, check (v >= x())); comment on sequence s is 'ids, by two';
