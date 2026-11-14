select json_set(:base, '$.name', 'Ada') as d, json_array_append(:list, '$', 1) as l
