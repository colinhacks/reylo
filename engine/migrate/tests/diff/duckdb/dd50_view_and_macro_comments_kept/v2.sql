create table t (id bigint, a varchar); create view v as select id, a from t; comment on view v is 'ids'; create macro f(x) as x + 2; comment on macro f is 'next';
