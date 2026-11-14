create macro x(a) as a > 0; create table t (id integer primary key, v integer, w integer, check (x(v))); create index t_v on t (v); comment on table t is 'one'; comment on index t_v is 'by v';
