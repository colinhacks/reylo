do $$ begin if not exists (select 1 from pg_roles where rolname = 'dp_reader') then create role dp_reader; end if; end $$;
