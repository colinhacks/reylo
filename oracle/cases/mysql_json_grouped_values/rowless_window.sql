select json_object('r', row_number() over (), 'l', lag(1) over (), 'n', count(*), 'f', first_value(any_value(u.name)) over ()) as o from users u where false
