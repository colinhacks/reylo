do $$ begin if not exists (select from pg_roles where rolname = 'diff_enum_user') then create role diff_enum_user; end if; end $$;
