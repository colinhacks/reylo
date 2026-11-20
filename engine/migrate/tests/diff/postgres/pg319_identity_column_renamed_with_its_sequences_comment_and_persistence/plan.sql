do $reylo$ begin execute format('comment on sequence %s is null', pg_get_serial_sequence('public.t', 'old')); end $reylo$;
do $reylo$ begin execute format('comment on sequence %s is null', pg_get_serial_sequence('public.t2', 'old')); end $reylo$;
do $reylo$ begin execute format('alter sequence %s set unlogged', pg_get_serial_sequence('public.t', 'old')); end $reylo$;
-- t.old is renamed new, keeping its rows. If they are two columns, write instead:
--   alter table t drop column old;
--   alter table t add column new bigint generated always as identity not null;
alter table t rename column old to new;
alter table t rename constraint t_old_not_null to t_new_not_null;
-- t1.old is renamed new, keeping its rows. If they are two columns, write instead:
--   alter table t1 drop column old;
--   alter table t1 add column new bigint generated always as identity not null;
alter table t1 rename column old to new;
alter table t1 rename constraint t1_old_not_null to t1_new_not_null;
-- t2.old is renamed new, keeping its rows. If they are two columns, write instead:
--   alter table t2 drop column old;
--   alter table t2 add column new bigint generated always as identity not null;
alter table t2 rename column old to new;
alter table t2 rename constraint t2_old_not_null to t2_new_not_null;
do $reylo$ begin execute format('alter sequence %s set logged', pg_get_serial_sequence('public.t', 'new')); end $reylo$;
do $reylo$ begin execute format('comment on sequence %s is %L', pg_get_serial_sequence('public.t', 'new'), 'ids'); end $reylo$;
do $reylo$ begin execute format('comment on sequence %s is %L', pg_get_serial_sequence('public.t2', 'new'), 'the second'); end $reylo$;
