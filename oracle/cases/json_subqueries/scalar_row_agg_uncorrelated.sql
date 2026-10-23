select (select coalesce(jsonb_agg(q), '[]') from posts q) as posts
