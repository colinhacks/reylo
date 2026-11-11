select p.id, l.note, l.at from people p cross join lateral (select v.note, v.at from visits v where v.person_id = p.id) l
