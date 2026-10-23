select case when m.id > 1 then jsonb_build_object('n', m.first_name) else null end as o from member m
