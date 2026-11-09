with w as (select array(select c from (select 1 as n) c) as arr union all select array[null::record]) select arr from w
