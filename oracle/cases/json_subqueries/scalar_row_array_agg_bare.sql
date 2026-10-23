select users.id, (select array_agg(q order by q.id) from posts q where q.user_id = users.id) as rows from users
