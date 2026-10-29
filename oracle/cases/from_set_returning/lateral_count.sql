select p.id, c.n, c.latest from people p cross join lateral (select count(*) as n, max(v.at) as latest from visits v where v.person_id = p.id) c
