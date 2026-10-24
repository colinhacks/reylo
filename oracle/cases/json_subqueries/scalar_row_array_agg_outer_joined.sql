select users.id, (select array_agg(q) from users v left join posts q on q.user_id = v.id where v.id = users.id) as rows from users
