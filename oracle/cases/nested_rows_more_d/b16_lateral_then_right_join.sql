select l.n from posts p cross join lateral (select p.id + 1 as n) l right join users u on false
