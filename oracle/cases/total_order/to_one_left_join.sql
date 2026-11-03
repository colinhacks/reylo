select u.name, pr.bio from users u left join profiles pr on pr.user_id = u.id order by u.id
