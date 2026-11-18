do $$ begin if not exists (select from pg_roles where rolname = 'diff_type_user') then create role diff_type_user; end if; end $$;
