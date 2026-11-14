do $$ begin if not exists (select 1 from pg_roles where rolname = 'owner_role') then create role owner_role; end if; end $$;
