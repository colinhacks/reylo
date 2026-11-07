select p.author_id, array(select c from (select max(p.id) as m, p.author_id as a) c) as arr from posts p group by rollup (p.author_id)
