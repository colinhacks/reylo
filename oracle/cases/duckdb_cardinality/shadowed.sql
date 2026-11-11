with accounts as (select * from accounts union all select * from accounts) select handle from accounts where id = :id
