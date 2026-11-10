select p.id, array(select c from (select id, big from comments where post_id = p.id and big > :min::bigint) c) as comments from posts p
