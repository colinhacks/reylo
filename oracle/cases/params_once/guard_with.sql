with input as (select :min::nullable(int) as min) select id from users, input where input.min is null or age > input.min
