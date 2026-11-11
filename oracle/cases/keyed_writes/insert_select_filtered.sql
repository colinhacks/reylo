with input as (select :id::int as id) insert into accounts (id, name) select input.id, 'x' from input where input.id > 0 returning id
