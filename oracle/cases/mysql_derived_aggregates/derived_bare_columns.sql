select (select coalesce(json_arrayagg(json_object('id', id, 'title', title)), json_array()) from (select id, title from posts order by id desc limit 5) p) as recent_posts
