select max(p.title) over (order by p.id) as running from posts p
