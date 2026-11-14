select p.author_id, array(select c from (select grouping(p.author_id) as g, p.author_id + 1 as a) c) as arr from posts p group by rollup (p.author_id)
