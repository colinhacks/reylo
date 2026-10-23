select users.id, (select count(*) from posts q where q.user_id = users.id) as n from users
