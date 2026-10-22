select coalesce(jsonb_agg(id), '[]'::jsonb) as ids from posts where false
