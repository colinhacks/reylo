with recursive cte as (select b.* from comments b where b.id = :id union all select b.* from comments b join cte c on b.parent_id = c.id) select * from cte
