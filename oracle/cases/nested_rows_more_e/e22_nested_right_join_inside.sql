select p.id, array(select c from (select x.big + 1 as k from comments x right join posts q on q.id = x.post_id where q.id = p.id) c) as arr from posts p
