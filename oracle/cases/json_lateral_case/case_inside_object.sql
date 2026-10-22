select jsonb_build_object('c', case when m.id > 1 then jsonb_build_object('n', m.first_name) end) as o from member m
