with input as (select :sort::text as sort) select id, name from users, input order by case when input.sort = 'name' then name end, id
