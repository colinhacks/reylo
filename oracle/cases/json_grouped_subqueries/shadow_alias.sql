select (select jsonb_build_object('m', max(u.id), 'c', count(*)) from posts u group by u.title order by u.title limit 1) as j from users u
