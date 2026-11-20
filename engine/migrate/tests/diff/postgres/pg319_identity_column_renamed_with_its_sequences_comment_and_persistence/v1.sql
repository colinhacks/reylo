create unlogged table t (id int primary key, old bigint generated always as identity);
comment on sequence t_old_seq is 'ids';
alter sequence t_old_seq set logged;
