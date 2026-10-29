select part from people p, regexp_split_to_table(p.csv, ',') part
