select p.id, json_build_object('names', (select coalesce(jsonb_agg(distinct t.name), '[]'::jsonb) from tags t where t.post_id = p.id)) as o from posts p
