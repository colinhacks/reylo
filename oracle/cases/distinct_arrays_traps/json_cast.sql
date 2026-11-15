select json_agg(distinct m) as moods, jsonb_agg(distinct m) as docs, array_agg(distinct m) as labels from j
