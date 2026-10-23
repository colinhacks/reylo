select users.id, (select coalesce(array_agg(q), '{}') from posts q where q.user_id = users.id) as rows from users
