select array(select x from (select null::int as id) a left join (select 2 as id) b using (id) as x) as arr
