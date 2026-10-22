select u.id, m.id as post, m.next from users u left join merged m on m.user_id = u.id + 1000
