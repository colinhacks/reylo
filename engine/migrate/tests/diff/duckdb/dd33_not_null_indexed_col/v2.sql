create table t (id bigint primary key, a varchar not null, b varchar); create index t_ab on t (a, b);
