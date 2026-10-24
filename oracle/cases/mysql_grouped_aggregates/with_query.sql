with t as (select user_id, id from posts) select user_id, max(id) as top from t group by user_id
