create table t (id integer primary key, a varchar check (a <> '')); comment on table t is 'new'; comment on column t.a is 'the a'; create index t_a on t (a);
