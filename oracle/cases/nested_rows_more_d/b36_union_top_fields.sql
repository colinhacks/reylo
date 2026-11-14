select array(select c from (select 1 as n) c) as arr union all select array[row(null::int)]
