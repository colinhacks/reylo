create table t_ident (id bigint generated always as identity, v integer);
grant usage on sequence t_ident_id_seq to diff_seq_user;
