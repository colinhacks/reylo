select u.id, m.next from users u left join (posts p join merged m on m.id = p.id) on p.user_id = u.id + 1000
