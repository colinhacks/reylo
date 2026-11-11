select * from (select id from posts union select post_id from tags) s where s.id = :id
