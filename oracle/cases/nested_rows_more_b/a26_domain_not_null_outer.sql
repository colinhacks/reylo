select array(select c from (select x.d, x.big from posts p left join comments x on false) c) as arr
