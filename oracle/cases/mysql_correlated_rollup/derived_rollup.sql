select u.id, u.name, s.n from users u join (select user_id, count(*) as n from posts group by user_id with rollup) s on s.user_id = u.id
