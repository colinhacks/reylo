alter table t rename constraint t_a_not_null to a_required;
do $reylo1$ begin execute format('alter sequence %s set logged', pg_get_serial_sequence('public."t$reylo$"', 'id')); end $reylo1$;
do $reylo$ begin execute format('alter sequence %s set logged', pg_get_serial_sequence('public.t', 'id')); end $reylo$;
do $reylo1$ begin execute format('comment on sequence %s is %L', pg_get_serial_sequence('public.t', 'id'), 'ids, quoted $reylo$ inside'); end $reylo1$;
