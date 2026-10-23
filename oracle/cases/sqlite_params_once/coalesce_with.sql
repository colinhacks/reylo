with input as (select :age::nullable(integer) as age) select id from users, input where users.age = coalesce(input.age, users.age)
