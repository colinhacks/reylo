select user_id, string_agg(title, ', ') as titles from posts group by user_id
