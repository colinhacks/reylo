create table t (id bigint, DATE date, UUID uuid, S varchar, p struct(LIST integer[]), primary key (id));
create index t_date on t(DATE);
create view v as select DATE as US, UUID from t where (S != 'x');
