with d as (delete from users returning id) select count(*) from d
