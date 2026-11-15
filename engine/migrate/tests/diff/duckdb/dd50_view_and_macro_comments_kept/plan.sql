drop view v;
create view v as select id, a from t;
drop macro f;
create macro f (x) as ((x + 2));
comment on macro f is 'next';
comment on view v is 'ids';
