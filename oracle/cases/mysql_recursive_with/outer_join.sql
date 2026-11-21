with recursive nums (n) as (select 1 union all select n + 1 from nums where n < 2) select u.id, nums.n from users u left join nums on nums.n = u.id
