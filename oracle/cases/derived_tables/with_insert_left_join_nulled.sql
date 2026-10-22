with i as (insert into tags values (-424242, 'a') returning *) select * from bar b left join i on false
