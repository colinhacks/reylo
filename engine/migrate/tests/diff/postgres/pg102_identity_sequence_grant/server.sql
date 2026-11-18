do $$ begin if not exists (select from pg_roles where rolname = 'diff_seq_user') then create role diff_seq_user; end if; end $$;
