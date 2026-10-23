with input as (select :q::vector(3) as q) select id, (embedding <#> q) * -1 as score from items, input where embedding <#> q < :threshold order by embedding <#> q
