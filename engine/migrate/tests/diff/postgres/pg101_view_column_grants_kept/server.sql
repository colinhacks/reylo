do $$ begin if not exists (select from pg_roles where rolname = 'diff_viewer') then create role diff_viewer; end if; end $$;
