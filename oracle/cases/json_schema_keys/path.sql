select doc #> '{owner,id}' as owner_id, doc #>> '{owner,id}' as owner_id_text, doc #> array['owner', 'email'] as email, doc #> '{}' as whole, doc #>> '{"kind"}' as kind from docs
