select json_array(1, 2) as pair, json_array(m.id, m.bio) as skipped, json_array(m.id, m.bio null on null) as kept, json_array() as empty from member m
