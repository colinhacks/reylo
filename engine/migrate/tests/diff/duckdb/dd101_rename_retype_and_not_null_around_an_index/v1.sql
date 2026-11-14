create table t (id integer primary key, a integer check (a > 0), n integer, s varchar, e varchar); create index te on t (e); comment on index te is 'by e';
