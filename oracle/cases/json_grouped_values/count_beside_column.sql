select jsonb_build_object('name', u.name, 'posts', count(p.id)) as j from users u left join posts p on p.user_id = u.id group by u.id
