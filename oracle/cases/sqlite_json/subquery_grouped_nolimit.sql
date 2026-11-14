select (select json_object('name', u.name, 'posts', count(*)) from users u join posts p on p.user_id = u.id group by u.id) as j
