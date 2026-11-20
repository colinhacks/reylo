do $$ begin create role diff_owner; exception when duplicate_object then null; end $$;
do $$ begin create role diff_super superuser; exception when duplicate_object then null; end $$;
