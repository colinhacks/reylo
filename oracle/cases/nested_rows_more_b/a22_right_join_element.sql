select array(select c from (select id from comments) c right join posts p on false) as arr
