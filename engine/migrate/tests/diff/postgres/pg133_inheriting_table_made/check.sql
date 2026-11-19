do $$ begin
  if exists (select from pg_attribute where attrelid = 'bindings_a'::regclass and attnum > 0 and attislocal and attinhcount > 0)
    or (select attislocal from pg_attribute where attrelid = 'bindings_b'::regclass and attname = 'kind') is not true then
    raise exception 'an inheriting table declares what it only inherits, or not what it declares';
  end if;
end $$;
