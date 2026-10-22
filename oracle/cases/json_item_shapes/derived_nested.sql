select jsonb_build_object('stats', s.stats) as j from (select jsonb_build_object('count', count(*)) as stats from posts) s
