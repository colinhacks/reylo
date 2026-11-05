with input as (select :id::int as id) update users u set name = 'x' from input where u.id = input.id returning u.id
