with input as (select :q::vector(3) as q) select id, embedding <-> q as distance from items, input order by embedding <-> q limit 5
