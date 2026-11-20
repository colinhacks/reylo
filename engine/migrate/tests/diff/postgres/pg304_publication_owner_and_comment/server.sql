do $$ begin if not exists (select from pg_roles where rolname = 'diff_pub_owner') then create role diff_pub_owner superuser; end if; end $$;
