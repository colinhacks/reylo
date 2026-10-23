with input as (select :q::nullable(varchar(40)) as q) select id from users, input where input.q is null or users.name = input.q
