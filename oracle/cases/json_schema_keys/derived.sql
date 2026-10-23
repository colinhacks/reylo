select x.name, x.owner_id from (select doc ->> 'name' as name, doc #> '{owner,id}' as owner_id from docs) x
