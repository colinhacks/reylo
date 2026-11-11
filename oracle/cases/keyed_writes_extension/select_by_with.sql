with input as (select :id::int as id) select u.id from users u, input where u.id = input.id
