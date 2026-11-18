do $$ begin if not exists (select from pg_roles where rolname = 'diff_anon') then create role diff_anon; end if; end $$;
do $$ begin if not exists (select from pg_roles where rolname = 'diff_auth') then create role diff_auth; end if; end $$;
