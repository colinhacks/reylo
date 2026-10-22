select case when id > 1 then null else jsonb_build_object('n', name) end, '{}'::jsonb as "case" from team
