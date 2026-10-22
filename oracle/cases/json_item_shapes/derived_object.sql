select s.stats from (select jsonb_build_object('count', count(*), 'latest', max(published_at)) as stats from posts) s
