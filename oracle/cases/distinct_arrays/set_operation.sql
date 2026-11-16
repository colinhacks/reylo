select array_agg(distinct name) as names from tags union all select array['z', 'z']
