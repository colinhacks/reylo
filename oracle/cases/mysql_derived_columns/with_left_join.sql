with d as (select user_id, id + 1 as next from posts) select u.id, d.next from users u left join d on d.user_id = u.id + 1000
