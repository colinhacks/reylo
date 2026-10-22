select (select json_arrayagg(json_object('id', p.id)) from (select id from posts limit 5) p) as recent_posts
