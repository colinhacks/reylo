select (with posts as (select id, nullif(title, title) as title from posts) select coalesce(json_arrayagg(json_object('id', id, 'title', title)), json_array()) from posts) as titles
