create table t (id bigint, a varchar); create view v as select id from t; comment on view v is 'ids'; create macro f(x) as x + 1; comment on macro f is 'next';
