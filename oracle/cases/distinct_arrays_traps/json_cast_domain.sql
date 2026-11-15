select jsonb_agg(m) as rows, json_agg(distinct m.f) as feelings from moods m
