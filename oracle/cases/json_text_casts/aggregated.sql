select jsonb_agg((j::text)::jsonb) as g, coalesce(jsonb_agg((n::text)::jsonb), '[]') as c from doc
