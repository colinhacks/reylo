with input as (select :id::int as id) delete from users u using input where u.id = input.id returning u.id
