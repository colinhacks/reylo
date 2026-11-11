select array(select c from (select id from comments) c full join (select 1 as z) z on false) as arr
