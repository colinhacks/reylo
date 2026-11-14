with comments as (select null::bigint as id) select (select array(select c from comments c)) as arr
