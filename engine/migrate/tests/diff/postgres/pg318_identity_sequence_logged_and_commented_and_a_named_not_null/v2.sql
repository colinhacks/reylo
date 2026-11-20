create unlogged table t (id bigint generated always as identity primary key, a int constraint a_required not null);
alter sequence t_id_seq set logged;
comment on sequence t_id_seq is 'ids';
