select jsonb_build_object('ids', coalesce(jsonb_agg(id), '{}'), 'titles', jsonb_agg(title)) as j from posts
