select p.author_id, array(select c from (select max(p.id) filter (where p.id > 100) as m) c) as arr from posts p group by p.author_id
