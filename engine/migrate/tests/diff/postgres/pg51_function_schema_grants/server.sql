do $$ begin if not exists (select 1 from pg_roles where rolname = 'reader') then create role reader; end if; end $$;
