with input as (select :h::halfvec(3) as h) select id, half <-> h as distance, half <=> h as cosine, sparse <-> :s as sparse_distance from items, input order by half <-> h limit 5
