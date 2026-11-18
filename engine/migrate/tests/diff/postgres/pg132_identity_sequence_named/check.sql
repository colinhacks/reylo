do $$ begin
  if pg_get_serial_sequence('t', 'id') is distinct from 'public.t_ids' or pg_get_serial_sequence('t', 'n') is distinct from 'public.t_counter'
    or pg_get_serial_sequence('u', 'id') is distinct from 'public."ids minvalue missing reserved"' or nextval(pg_get_serial_sequence('u', 'id')) <> 3 then
    raise exception 'an identity''s sequence is not under the name it was given';
  end if;
end $$;
