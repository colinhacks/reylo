select date_format(p.published_at, '%Y') as y, count(*) as n, max(p.id) as top from posts p group by date_format(p.published_at, '%Y')
