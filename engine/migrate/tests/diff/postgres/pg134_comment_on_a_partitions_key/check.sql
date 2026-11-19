do $$ begin
  if (select obj_description(oid, 'pg_constraint') from pg_constraint where conrelid = 'events_leaf'::regclass and contype = 'p') is distinct from 'child key' then
    raise exception 'the partition''s key lost its comment';
  end if;
end $$;
