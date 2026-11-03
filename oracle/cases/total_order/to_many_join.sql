select u.name, p.title from users u join posts p on p.author_id = u.id order by u.id
