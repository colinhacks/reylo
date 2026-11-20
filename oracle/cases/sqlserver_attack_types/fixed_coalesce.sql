select coalesce(cn, 'a') as c1, isnull(cn, 'a') as c2, coalesce(bn, 0x01) as b1, isnull(bn, 0x01) as b2, iif(id < 0, b, 0x01) as b3 from fx
