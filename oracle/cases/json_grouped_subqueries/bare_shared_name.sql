select (select jsonb_build_object('m', max(title), 'c', count(*), 'a', jsonb_agg(title)) from posts p group by p.user_id order by p.user_id limit 1) as j from users u
