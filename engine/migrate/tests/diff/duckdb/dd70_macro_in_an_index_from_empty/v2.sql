create macro norm(x) as lower(x); create table t (e varchar); create index t_e on t (norm(e));
