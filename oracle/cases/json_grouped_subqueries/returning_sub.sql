update posts set score = score returning (select jsonb_build_object('c', count(*), 'm', max(q.n), 'a', jsonb_agg(q.title)) from posts q group by q.user_id order by 1 limit 1) as j
