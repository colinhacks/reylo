create table t (id bigint primary key, a varchar, active boolean not null default true); create index t_a on t (a);
