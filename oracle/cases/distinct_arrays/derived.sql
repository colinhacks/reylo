select s.names from (select array_agg(distinct name) as names from tags) s
