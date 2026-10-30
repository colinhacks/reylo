update users set name = :name where id = :id returning with (old as o, new as n) o.name as old_name, n.name as new_name
