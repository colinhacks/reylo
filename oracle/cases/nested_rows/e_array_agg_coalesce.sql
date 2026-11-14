select p.id, (select coalesce(array_agg(c), '{}') from (select id, big from comments where post_id = p.id) c) as comments from posts p
