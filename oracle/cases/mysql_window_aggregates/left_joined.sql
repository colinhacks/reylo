select u.name, sum(p.id) over (partition by u.id) as total from users u left join posts p on p.user_id = u.id
