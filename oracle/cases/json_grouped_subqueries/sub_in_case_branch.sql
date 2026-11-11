select case when u.id > 0 then (select jsonb_build_object('a', jsonb_agg(p.id), 'c', count(*)) from posts p where p.user_id = u.id group by p.tag order by 1 limit 1) end as j from users u
