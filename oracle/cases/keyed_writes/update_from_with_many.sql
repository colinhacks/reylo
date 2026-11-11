with input as (select id from accounts) update users u set name = 'x' from input where u.id = input.id returning u.id
