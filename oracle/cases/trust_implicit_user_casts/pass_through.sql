select array_agg(b) as a, to_jsonb(min(id)) as j, count(b) as n from tb
