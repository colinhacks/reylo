with input as (select :q::vector(3) as q) select id, embedding <=> q as distance, case when vector_dims(draft) = 3 then draft <=> q end as draft_distance from items, input order by 2 limit 3
