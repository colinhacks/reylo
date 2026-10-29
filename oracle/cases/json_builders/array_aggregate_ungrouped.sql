select array_to_json(array_agg(p.title)) as titles from post p
