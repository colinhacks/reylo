select u.id, m.next from merged m right join users u on m.user_id = u.id + 1000
