with input as (select :id::int as id) delete from notes n using input where n.id = input.id returning n.id
