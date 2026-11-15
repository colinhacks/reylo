create table t (id integer primary key, s varchar check (s not in (')', ',', '(')), n integer);
