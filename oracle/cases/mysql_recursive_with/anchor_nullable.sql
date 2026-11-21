with recursive r (id, nickname) as (select id, nickname from users where id = 1 union all select u.id, u.nickname from users u join r on u.id = r.id + 1) select id, nickname from r
