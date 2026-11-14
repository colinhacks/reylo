create sequence ids; create macro next_id() as nextval('ids'); create table t (id bigint primary key default next_id(), n integer);
