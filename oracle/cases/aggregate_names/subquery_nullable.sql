select (select string_agg(p.title, ', ') from posts p where p.user_id = u.id) as titles from users u
