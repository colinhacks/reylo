create unlogged table t (id int primary key, new bigint generated always as identity);
comment on sequence t_new_seq is 'ids';
alter sequence t_new_seq set logged;
