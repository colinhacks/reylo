select array(select u from (select unnest([1, null]) as x) u) as arr
