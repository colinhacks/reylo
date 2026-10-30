select user_id from posts except select id from users where id = :id
