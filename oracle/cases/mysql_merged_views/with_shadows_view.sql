with merged as (select id, id + 1 as next from users) select u.id, m.next from users u left join merged m on m.id = u.id + 1000
