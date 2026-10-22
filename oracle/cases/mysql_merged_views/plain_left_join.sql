select u.id, v.next from users u left join plain v on v.user_id = u.id + 1000
