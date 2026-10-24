select count(*) over () as n, total(p.id) over () as t from posts p
