select * from (select id as post_id, title from posts) s where s.post_id = :id
