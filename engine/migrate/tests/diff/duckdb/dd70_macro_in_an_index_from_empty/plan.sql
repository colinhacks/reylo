create table t (e varchar);
create macro norm (x) as (lower(x));
create index t_e on t((norm(e)));
