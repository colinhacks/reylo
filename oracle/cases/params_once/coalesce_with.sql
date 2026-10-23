with input as (select :age::nullable(int) as age) select id from users, input where users.age = coalesce(input.age, users.age)
