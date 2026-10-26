with flags as (select id, title from posts) select * from flags where flags.id = :id
