select array(select r from (select 99 as x) x left join replies r on r.id = x.x) as rs
