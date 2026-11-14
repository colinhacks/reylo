create macro x(v) as v + 1; create table t (id integer primary key, v integer, check (x(v) > 0)); create index t_x on t ((x(v))); comment on index t_x is 'by x';
