insert into accounts (id, name) select :id::int, 'x' returning id
