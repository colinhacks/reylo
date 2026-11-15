select jsonb_agg(p) as everyone, json_build_object('home', (select home from people where id = 1)) as first from people p
