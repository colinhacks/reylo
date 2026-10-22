select d.id, d.next from (select u.id, m.next from users u left join merged m on m.user_id = u.id + 1000) d
