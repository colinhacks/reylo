select (with r as (select id, title from posts) select json_arrayagg(json_object('id', id, 'body', null)) from r) as recent_posts
