select u.id, pr.bio from users u left join profiles pr on pr.user_id = u.id
