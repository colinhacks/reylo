select p.id, array(select c from (select lag(p.id) over () as l, p.id + 1 as n) c) as arr from posts p
