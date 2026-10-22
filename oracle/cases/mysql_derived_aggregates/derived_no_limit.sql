select (select coalesce(json_arrayagg(json_object('id', p.id, 'title', p.title)), json_array()) from (select id, title from posts) p) as all_posts
