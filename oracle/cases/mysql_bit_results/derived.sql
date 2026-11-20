select d.digits, d.mask, (d.most) as most from (select greatest(mask, mask) as digits, mask, nullif(mask, b'0') as most from flags) d
