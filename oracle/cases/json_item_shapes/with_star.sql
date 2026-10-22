with s as (select jsonb_build_object('id', id) as j from posts) select * from s
