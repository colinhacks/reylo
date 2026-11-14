select array(select c from posts p left join (users u join (select id from comments) c on true) on false) as arr
