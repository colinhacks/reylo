select p.id, array_agg(c) as arr from posts p left join comments c on c.post_id = p.id group by p.id
