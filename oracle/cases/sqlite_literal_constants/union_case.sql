select 'user' as kind, id, case when x > 0 then 'pos' when x < 0 then 'neg' end as sign from t union all select 'org', id, 'zero' from t
