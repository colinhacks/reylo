select array(select c from (select * from posts natural full join users) c) as arr
