drop view v;
create view v as select id, n, (n + 1) as m from t;
comment on column v.id is 'the id';
