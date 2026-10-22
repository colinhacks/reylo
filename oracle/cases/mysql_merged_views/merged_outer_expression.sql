select u.id, m.next + 1 as after from users u left join merged m on m.user_id = u.id + 1000
