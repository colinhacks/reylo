select p.author_id, array(select c from (select p.author_id as a, p.author_id + 1 as b) c) as arr from posts p group by rollup (p.author_id)
