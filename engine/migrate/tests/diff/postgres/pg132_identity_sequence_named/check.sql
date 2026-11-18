do $$ begin
  if pg_get_serial_sequence('t', 'id') is distinct from 'public.t_ids' or pg_get_serial_sequence('t', 'n') is distinct from 'public.t_counter' then
    raise exception 'an identity''s sequence is not under the name it was given';
  end if;
end $$;
