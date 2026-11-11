select pr.user_id, u.name from users u left join profiles pr on pr.user_id = u.id
