select p.id, array(select c from (select id, jsonb_build_object('big', big, 'b', body) as doc from comments where post_id = p.id) c) as comments from posts p
