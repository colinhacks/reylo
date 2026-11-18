do $$ begin
  if not exists (select from pg_constraint where conrelid = 'users'::regclass and contype = 'n' and conname = 'users_name_not_null') then
    raise exception 'the not null kept the name of the column it was made on';
  end if;
end $$;
