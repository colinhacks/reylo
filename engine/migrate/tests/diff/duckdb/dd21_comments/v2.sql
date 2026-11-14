create table t (id bigint primary key, a varchar); comment on table t is 'new'; create view v as select * from t; comment on view v is 'every t';
