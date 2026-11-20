select u.id, (select top 1 p.title from posts p where p.user_id = u.id order by p.id for json path, without_array_wrapper) as first_post from users u
