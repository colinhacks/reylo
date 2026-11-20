do $$ begin if not exists (select from pg_roles where rolname = 'diff_seq_reader') then create role diff_seq_reader; end if; end $$;
