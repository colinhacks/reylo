select array(select c from (select x.id, l.n from comments x left join lateral (select x.big + 1 as n where false) l on true) c) as arr
