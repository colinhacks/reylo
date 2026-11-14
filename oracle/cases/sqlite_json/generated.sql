select json_array(j, js, jj, label) as v, json_object('j', j, 'label', label) as o from docs
