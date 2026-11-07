select array(select j from ((select 1 as n) a left join (select 2 as m) b on false) j) as arr
