select case when id > 1 then jsonb_build_object('a', 1) else jsonb_build_object('b', 2) end as o from member
