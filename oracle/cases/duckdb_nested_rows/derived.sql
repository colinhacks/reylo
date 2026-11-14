select p.id, array(select c from (select id, body, big from comments where post_id = p.id) c) as comments from posts p
