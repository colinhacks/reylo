select case when total > 15 then 'big' else 'small' end as bucket, count(*) as n from orders group by rollup (bucket)
