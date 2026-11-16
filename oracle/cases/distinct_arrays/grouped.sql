select post_id, array_agg(distinct name) as names, array_agg(distinct score) as scores, array_agg(distinct f) as fs, array_agg(distinct at) as ats from tags group by post_id
