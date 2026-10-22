select coalesce(jsonb_agg(id), '{}') as ids from posts where false
