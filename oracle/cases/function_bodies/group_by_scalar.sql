select count_of(p.id) as n, count(*) as c from posts p group by count_of(p.id)
