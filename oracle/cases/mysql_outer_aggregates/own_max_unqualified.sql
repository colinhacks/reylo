select p.id, (select max(created_at) from comments where post_id = p.id) as last from posts p
