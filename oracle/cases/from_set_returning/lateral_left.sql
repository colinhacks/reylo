select p.id, c.n from people p left join lateral (select count(*) as n from visits v where v.person_id = p.id group by v.at) c on true
