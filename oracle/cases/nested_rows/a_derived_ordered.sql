select p.id, array(select c from (select id, body from comments where post_id = p.id order by id limit 2) c) as comments from posts p
