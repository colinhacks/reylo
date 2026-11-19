select array_agg(y::int[] order by id) as stacked from lists
