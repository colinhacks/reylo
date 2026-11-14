select (select jsonb_build_object('name', u.name, 'posts', count(*)) from users u join posts p on p.user_id = u.id group by u.id order by count(*) desc limit 1) as j
