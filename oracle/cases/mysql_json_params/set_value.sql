update profiles set doc = json_set(doc, '$.name', :name) where id = :id
