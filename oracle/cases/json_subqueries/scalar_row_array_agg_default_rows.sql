select users.id, (select coalesce(array_agg(q), array[row(0, 0, null)::posts]) from posts q where q.user_id = users.id) as rows from users
