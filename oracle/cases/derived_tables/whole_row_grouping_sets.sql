select b.id, (select array_agg(t) from (select id from tags) t) as ts from bar b group by grouping sets ((b.id), ())
