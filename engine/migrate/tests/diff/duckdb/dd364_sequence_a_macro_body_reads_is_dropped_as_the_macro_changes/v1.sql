create sequence s; create macro next_id() as nextval('s'); create table t (id bigint primary key default next_id(), n integer);
