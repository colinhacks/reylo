with recursive r (id, depth) as (select id, 0 from posts union all select id, depth + 1 from r where depth < 2) select id from r where r.id = :id
