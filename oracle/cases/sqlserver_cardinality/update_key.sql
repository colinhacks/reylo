update customers set name = :name output deleted.name as old_name, inserted.name as new_name where id = :id
