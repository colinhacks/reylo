do $$ begin create role diff_reader; exception when duplicate_object then null; end $$;
