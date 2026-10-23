select json_object_agg(id, title) as titles, jsonb_object_agg(id, title) as titles_b from posts
