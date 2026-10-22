select m.id, m.next, u.name from merged m left join users u on u.id = m.user_id
