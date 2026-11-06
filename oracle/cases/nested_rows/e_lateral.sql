select p.id, l.comments from posts p left join lateral (select array(select c from (select id, big from comments where post_id = p.id) c) as comments) l on true
