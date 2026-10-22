with a as (select user_id, jsonb_build_object('id', id) as j from posts), b as (select user_id, jsonb_agg(a.j) as js from a group by user_id) select jsonb_build_object('js', b.js) as k from b
