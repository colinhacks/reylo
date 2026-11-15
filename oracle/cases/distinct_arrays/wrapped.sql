select coalesce(array_agg(distinct name), '{}') as names, array_agg(distinct name order by name) filter (where name > 'a') as later from tags
