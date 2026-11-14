select json_extract(:doc, '$.a') as a, json_length(:other) as n, json_type(:third) as t
