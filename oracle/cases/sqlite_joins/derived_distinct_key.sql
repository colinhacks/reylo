select * from (select distinct user_id from posts) s where s.user_id = :u
