with s (j) as (select jsonb_build_object('id', id) from posts) select j from s
