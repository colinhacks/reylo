with a as (select id from flags), b as (select p.id from posts p) select * from b where b.id = :id
