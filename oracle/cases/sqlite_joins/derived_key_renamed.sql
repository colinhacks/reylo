select * from (select id as user_id from users) s where s.user_id = :id
