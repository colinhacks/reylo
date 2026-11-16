select jsonb_agg(distinct name) as names, json_agg(distinct score order by score) as scores, jsonb_agg(distinct t) as rows from tags t
