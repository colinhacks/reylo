select p.id, array_agg(c) as arr from posts p left join (select id, big from comments) c on c.id = p.id + 100 group by p.id
