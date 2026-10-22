select u.id, v.next from users u left join temp v on v.user_id = u.id + 1000
