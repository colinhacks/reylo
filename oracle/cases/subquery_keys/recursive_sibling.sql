with recursive a (id, depth) as (select id, 0 from posts union all select id, depth + 1 from a where depth < 2), b as (select * from a) select * from b where b.id = :id
