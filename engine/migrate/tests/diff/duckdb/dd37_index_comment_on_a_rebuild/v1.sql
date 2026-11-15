create table t (id bigint primary key, a varchar); create index t_a on t (a); comment on index t_a is 'lookup';
