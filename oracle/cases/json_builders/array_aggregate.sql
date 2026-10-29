select m.id, array_to_json(array_agg(p.title)) as titles from member m join post p on p.member_id = m.id group by m.id
