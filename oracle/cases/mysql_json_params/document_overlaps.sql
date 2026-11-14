select id from profiles where json_overlaps(doc, :other) or json_contains(:target, '1')
