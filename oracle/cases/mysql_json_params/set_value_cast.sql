update profiles set doc = json_set(doc, '$.name', :name::json) where id = :id
