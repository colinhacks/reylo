create macro norm(x) as upper(x); create table t (e varchar); create index t_e on t (norm(e)); comment on index t_e is 'by e';
