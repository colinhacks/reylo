select (select coalesce(array_agg(p), '{}') from posts p where p.id = 1) as rows from users
