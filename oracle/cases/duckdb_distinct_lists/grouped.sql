select post_id, list(distinct name) as names, array_agg(distinct score order by score) as scores from tags group by post_id
