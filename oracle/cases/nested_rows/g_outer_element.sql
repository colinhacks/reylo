select array(select c from (select 1 as one) x left join (select 1 as n where false) c on true) as els
