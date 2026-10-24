with posts as (select 1::bigint as id) select v.title from public_posts v where v.id = :id
