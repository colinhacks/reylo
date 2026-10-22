select p.id, (with posts as (select id, email from users) select json_object('a', posts.email, 'b', p.title) from posts limit 1) as o from posts p
