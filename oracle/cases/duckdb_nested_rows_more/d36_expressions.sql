select array(select c from (select x.id, x.big + 1 as b, nullif(x.big, 5) as nb, x.big / 0 as dz from comments x) c) as arr
