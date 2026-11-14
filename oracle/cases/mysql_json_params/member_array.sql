select id from profiles where json_extract(doc, '$.name') member of (:names)
