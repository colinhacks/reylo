select array(select c from (select l.* from comments x cross join lateral (select 1 as v, x.body as v) l) c) as arr
