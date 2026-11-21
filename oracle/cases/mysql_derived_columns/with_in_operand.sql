with x as (select email as v from users where email is not null) (with x as (select null as v) select 'a' as c from x) union all select d.v from (select v from x) d
