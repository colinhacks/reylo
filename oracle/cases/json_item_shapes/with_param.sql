with s as (select jsonb_build_object('id', id, 'p', :p::text) as j from posts) select j from s
