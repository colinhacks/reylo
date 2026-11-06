select j.a, j.b, j.c
from json_table('[{"a": "xyzxyzxyz", "b": 123.456, "c": "2026-01-01 12:00:00.123456"}, {"a": "ab", "b": 99999, "c": "2026-01-01"}]', '$[*]' columns (a varchar(5) path '$.a', b decimal(4,1) path '$.b', c datetime(2) path '$.c')) j
