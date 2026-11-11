select (select jsonb_build_object('k', d.x, 'c', count(*)) from posts p group by p.tag order by 1 limit 1) as j from (select id as x from users union all select null) d
