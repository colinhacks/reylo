select (select jsonb_build_object('c', count(*), 'm', max(p.n)) as tag from posts p group by tag order by 1 limit 1) as j
