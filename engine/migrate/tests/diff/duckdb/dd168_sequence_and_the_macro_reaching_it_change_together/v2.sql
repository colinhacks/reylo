create sequence ids increment by 2; create macro next_id() as nextval('ids') + 0; create table t (id bigint primary key default next_id(), n integer);
