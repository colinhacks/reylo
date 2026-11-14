select array(select c from (select 1 as n) c positional join (select 1 as m, 2 as k) d) as arr
