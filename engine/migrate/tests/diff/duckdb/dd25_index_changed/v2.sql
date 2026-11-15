create table t (id bigint primary key, a varchar, b varchar); create index t_ab on t (a, b); create unique index t_b on t (b);
