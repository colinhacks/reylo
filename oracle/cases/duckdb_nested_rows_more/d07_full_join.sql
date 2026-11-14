select array(select c from (select 1 as n) c full join (select 1 as z) z on false) as arr
