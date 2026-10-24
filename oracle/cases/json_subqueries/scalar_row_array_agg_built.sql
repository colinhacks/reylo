select users.id, (select coalesce(array_agg(row(q.id, q.user_id, q.title)::posts), '{}') from posts q where q.user_id = users.id) as rows from users
