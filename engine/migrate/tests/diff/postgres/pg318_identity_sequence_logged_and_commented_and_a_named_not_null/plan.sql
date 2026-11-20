alter table t rename constraint t_a_not_null to a_required;
alter sequence t_id_seq set logged;
comment on sequence t_id_seq is 'ids';
