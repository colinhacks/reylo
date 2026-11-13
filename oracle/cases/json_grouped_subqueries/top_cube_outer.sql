select u.id, (select jsonb_build_object('k', u.id, 'c', count(*)) from posts p group by p.user_id order by 1 limit 1) as j from users u group by cube (u.id)
