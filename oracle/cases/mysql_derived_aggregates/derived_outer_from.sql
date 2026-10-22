select coalesce(json_arrayagg(json_object('id', p.id, 'title', p.title)), json_array()) as recent_posts from (select id, title from posts order by id desc limit 5) p
