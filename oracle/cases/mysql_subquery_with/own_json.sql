select (with r as (select id, title from posts order by id desc limit 5) select coalesce(json_arrayagg(json_object('id', r.id, 'title', r.title)), json_array()) from r) as recent_posts
