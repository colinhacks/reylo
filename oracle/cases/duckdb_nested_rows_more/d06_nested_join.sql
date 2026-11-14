select array(select c from (select 1 as z) z left join ((select 1 as n) c cross join (select 2 as w) w) on false) as arr
