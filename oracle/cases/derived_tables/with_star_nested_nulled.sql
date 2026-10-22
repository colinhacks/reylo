with t as (select id as tag_id, label from tags), u as (select * from t) select * from bar b left join u on false
