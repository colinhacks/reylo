with recent as (select id, title from posts order by id desc limit 5) select (select coalesce(json_arrayagg(json_object('id', id, 'title', title)), json_array()) from recent) as recent_posts
