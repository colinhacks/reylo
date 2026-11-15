do $$ begin if not exists (select 1 from pg_roles where rolname = 'col_reader') then create role col_reader; end if; end $$;
