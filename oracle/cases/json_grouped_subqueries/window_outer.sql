select (select jsonb_build_object('k', d.l, 'c', count(*)) from posts p group by p.tag order by 1 limit 1) as j from (select lag(u.id) over (order by u.id) as l from users u) d
