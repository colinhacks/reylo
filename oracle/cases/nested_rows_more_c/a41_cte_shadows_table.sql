with comments as (select null::bigint as id, null::bigint as big) select array(select c from comments c) as arr
