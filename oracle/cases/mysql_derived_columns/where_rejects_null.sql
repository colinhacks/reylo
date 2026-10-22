select u.id, d.next from users u left join (select user_id, id + 1 as next from posts) d on d.user_id = u.id where d.next > 0
