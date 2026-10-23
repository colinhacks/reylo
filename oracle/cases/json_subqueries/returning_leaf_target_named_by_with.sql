with users as (select 'a' as p) update users set name = 'x' where p is null returning p || 'x' as e
