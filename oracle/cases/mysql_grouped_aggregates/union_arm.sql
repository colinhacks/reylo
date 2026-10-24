select user_id, max(id) as top from posts group by user_id union all select 0, 1
