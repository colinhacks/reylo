select id, seg_lower(span) as low, seg_upper(span) as high, span && :s as overlaps from stock
