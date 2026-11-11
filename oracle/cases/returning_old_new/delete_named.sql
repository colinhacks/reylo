delete from users where name = :name and id = :id returning with (new as after) old.name as old_name, after.name as new_name, name
