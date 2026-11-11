select (select jsonb_build_object('c', count(*), 'm', max(p.n)) from posts p group by p.user_id order by 1 limit 1) as j from users u group by ()
