select sum(p.id) over () as total, avg(p.id) over (order by p.id) as mean from posts p
