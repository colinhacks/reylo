select (select u.id, p.id as pid from users u join posts p on p.user_id = u.id for json auto) as people
