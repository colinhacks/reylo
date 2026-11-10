with x as (select id, big from comments) select array(select x from x) as comments
