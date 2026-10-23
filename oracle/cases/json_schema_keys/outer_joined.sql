select e.id, x.kind from events e left join (select id, doc ->> 'kind' as kind from docs) x on x.id = e.id
