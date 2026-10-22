select u.id, d.next, d.title from users u left join (select user_id, id + 1 as next, title from posts) d on d.user_id = u.id + 1000
