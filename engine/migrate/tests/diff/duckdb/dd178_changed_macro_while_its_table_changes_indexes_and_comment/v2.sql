create macro x(a) as a >= 0; create table t (id integer primary key, v integer, w integer, check (x(v))); create index t_w on t (w); comment on table t is 'two'; comment on index t_w is 'by w';
