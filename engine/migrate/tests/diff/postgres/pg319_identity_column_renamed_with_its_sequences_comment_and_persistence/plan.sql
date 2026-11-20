comment on sequence t_old_seq is null;
alter sequence t_old_seq set unlogged;
-- t.old is renamed new, keeping its rows. If they are two columns, write instead:
--   alter table t drop column old;
--   alter table t add column new bigint generated always as identity not null;
alter table t rename column old to new;
alter table t rename constraint t_old_not_null to t_new_not_null;
alter sequence t_old_seq rename to t_new_seq;
alter sequence t_new_seq set logged;
comment on sequence t_new_seq is 'ids';
