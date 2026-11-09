select array(select c from (select 1 as n) c semi join (select 1 as n) d on c.n = d.n) as arr
