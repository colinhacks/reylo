select d.user_id, d.top from (select user_id, max(id) as top from posts group by user_id) d
