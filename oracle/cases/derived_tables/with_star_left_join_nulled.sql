with t as (select id as tag_id, label from tags) select * from bar b left join t on false
