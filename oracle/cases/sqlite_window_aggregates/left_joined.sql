select u.name, max(p.id) over (partition by u.id) as top from users u left join posts p on p.user_id = u.id
