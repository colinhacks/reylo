with input as (select :id::int as id, :name::text as name) update users u set name = input.name from input where u.id = input.id and u.name <> input.name returning u.id
