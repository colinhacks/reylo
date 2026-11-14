select array(select c from (select x.big from posts p left join comments x on false) c) as arr
