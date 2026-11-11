select (select jsonb_build_object('k', d.m, 'c', count(*)) from posts p group by p.tag order by 1 limit 1) as j from (select max(id) as m from users) d
