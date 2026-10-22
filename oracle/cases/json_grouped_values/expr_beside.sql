select jsonb_build_object('upper', upper(u.name), 'n', count(p.id)) as j from users u left join posts p on p.user_id = u.id group by u.id
