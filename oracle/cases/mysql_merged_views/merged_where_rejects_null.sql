select u.id, m.next from users u left join merged m on m.user_id = u.id where m.next > 0
