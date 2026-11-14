comment on column t.a is null;
create view v as select * from t;
comment on table t is 'new';
comment on view v is 'every t';
