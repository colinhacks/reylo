update profiles set doc = json_array_append(doc, '$.tags', cast(:tag as json)) where id = :id
