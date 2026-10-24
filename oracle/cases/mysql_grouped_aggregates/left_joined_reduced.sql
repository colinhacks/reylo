select u.id, max(p.title) as top from users u left join posts p on p.user_id = u.id where p.id > 0 group by u.id
