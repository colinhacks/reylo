with w as materialized (select x.id, x.body from comments x) select array(select w from w) as arr
