select l.n, l.t from users u left join posts p on false cross join lateral (select p.id + 1 as n, upper(p.title) as t) l
