select array(select c from (select * from posts p left join comments x using (id)) c) as arr
