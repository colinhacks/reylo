select jt.* from people p, json_table(p.list, '$[*]' columns (n for ordinality, v int path '$', nested path '$[*]' columns (m for ordinality))) jt
