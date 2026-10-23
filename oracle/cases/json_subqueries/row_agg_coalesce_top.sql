select coalesce(jsonb_agg(q), '[]') as posts from posts q
