with x as (select id, doc ->> 'kind' as kind, doc -> 'owner' -> 'id' as owner_id from docs) select kind, owner_id from x
