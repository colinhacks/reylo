select u.id, (select jsonb_build_object('score', p.score, 'n', count(*)) from posts p where p.user_id = u.id group by p.score order by count(*) desc limit 1) as j from users u
