select string_agg(title, ', ') as titles, array_agg(id) as ids, bool_and(true) as b, jsonb_object_agg(id, title) as o from drafts
