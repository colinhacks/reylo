with s as (select jsonb_build_object('id', id) as j from posts union all select jsonb_build_object('name', name) from users) select j from s
