select c.name, (with kids as (select id from categories k where k.parent_id = c.id) select count(*) from kids) as kids from categories c
