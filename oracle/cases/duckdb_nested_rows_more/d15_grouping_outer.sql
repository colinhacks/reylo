select t.a, array(select c from (select t.a as x) c) as arr from (select 1 as a) t group by rollup (t.a)
