with a as (select 1 as x union select 2) select e.kind from events e, a
